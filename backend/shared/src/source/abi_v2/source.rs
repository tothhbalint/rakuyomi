use std::{collections::HashMap, io::Cursor, path::Path, time::Duration};

use anyhow::{bail, Context, Result};
use chrono::{TimeZone, Utc};
use serde::{de::DeserializeOwned, Serialize};
use url::Url;
use wasmi::{Engine, Instance, Linker, Module, Store};

use crate::{
    settings::{Settings, SourceSettingValue},
    source::{
        model::{
            Chapter, Manga, MangaContentRating, MangaViewer, Page, PublishingStatus,
            SettingDefinition,
        },
        SourceManifest,
    },
};

use super::{
    model::{
        Chapter as AidokuChapter, ContentRating, Manga as AidokuManga, MangaStatus, PageContent,
        Viewer,
    },
    settings::parse_settings,
    store::{DefaultValue, NetRequest, StoreItem, WasmStore},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiV2Detection {
    Modern,
    Legacy,
}

pub struct AbiV2Source {
    store: Store<WasmStore>,
    instance: Instance,
    manifest: SourceManifest,
    setting_definitions: Vec<SettingDefinition>,
}

impl AbiV2Source {
    pub fn detect(engine: &Engine, wasm: &[u8]) -> Result<AbiV2Detection> {
        let module = Module::new_streaming(engine, Cursor::new(wasm))
            .context("failed to compile source module")?;
        let modern = module
            .exports()
            .any(|export| export.name() == "get_search_manga_list");
        Ok(if modern {
            AbiV2Detection::Modern
        } else {
            AbiV2Detection::Legacy
        })
    }

    pub fn from_aix_file(path: &Path, settings: Settings) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("couldn't open {}", path.display()))?;
        let mut archive = zip::ZipArchive::new(file)
            .with_context(|| format!("couldn't open source archive {}", path.display()))?;

        let manifest: SourceManifest = {
            let manifest_file = archive
                .by_name("Payload/source.json")
                .context("while loading source.json")?;
            serde_json::from_reader(manifest_file).context("failed to parse source.json")?
        };

        let settings_json = archive
            .by_name("Payload/settings.json")
            .ok()
            .and_then(|file| serde_json::from_reader::<_, serde_json::Value>(file).ok());

        let (setting_definitions, mut defaults) = match settings_json.as_ref() {
            Some(value) => parse_settings(&value.to_string()),
            None => (Vec::new(), HashMap::new()),
        };

        // Overlay user-stored settings. The stored values are the ones exposed by
        // rakuyomi's settings UI, so they can only be booleans or strings.
        if let Some(stored) = settings.source_settings.get(&manifest.info.id) {
            for (key, value) in stored {
                let value = match value {
                    SourceSettingValue::Bool(value) => DefaultValue::Bool(*value),
                    SourceSettingValue::String(value) => DefaultValue::String(value.clone()),
                };
                defaults.insert(key.clone(), value);
            }
        }

        // Sources use the `languages` default to decide which chapter languages
        // to fetch. Fall back to the languages declared by the source itself.
        let languages = if settings.languages.is_empty() {
            if manifest.info.languages.iter().any(|lang| lang == "en") {
                vec!["en".to_string()]
            } else {
                manifest.info.languages.iter().take(1).cloned().collect()
            }
        } else {
            settings.languages.clone()
        };
        if !languages.is_empty() {
            defaults
                .entry("languages".to_string())
                .or_insert_with(|| DefaultValue::StringArray(languages));
        }

        let wasm = {
            let mut file = archive
                .by_name("Payload/main.wasm")
                .context("while loading main.wasm")?;
            let mut bytes = Vec::with_capacity(file.size() as usize);
            std::io::Read::read_to_end(&mut file, &mut bytes)?;
            bytes
        };

        let engine = Engine::default();
        let module = Module::new_streaming(&engine, Cursor::new(&wasm))
            .with_context(|| format!("failed loading module from {}", path.display()))?;

        let wasm_store = WasmStore::new(
            manifest.info.id.clone(),
            defaults,
            tokio_util::sync::CancellationToken::new(),
        );
        let mut store = Store::new(&engine, wasm_store);
        let mut linker = Linker::new(&engine);
        super::register_imports(&mut linker)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .with_context(|| {
                format!(
                    "failed creating instance when loading from {}",
                    path.display()
                )
            })?
            .start(&mut store)?;

        // The current source SDK exports its constructor as `start`.
        if let Ok(start) = instance.get_typed_func::<(), ()>(&mut store, "start") {
            start.call(&mut store, ())?;
        }

        Ok(Self {
            store,
            instance,
            manifest,
            setting_definitions,
        })
    }

    pub fn manifest(&self) -> &SourceManifest {
        &self.manifest
    }

    pub fn setting_definitions(&self) -> &[SettingDefinition] {
        &self.setting_definitions
    }

    pub fn set_cancellation_token(&mut self, token: tokio_util::sync::CancellationToken) {
        self.store.data_mut().cancellation_token = token;
    }

    fn memory(&self) -> Result<wasmi::Memory> {
        match self.instance.get_export(&self.store, "memory") {
            Some(wasmi::Extern::Memory(memory)) => Ok(memory),
            _ => bail!("source has no exported memory"),
        }
    }

    fn read_result(&mut self, pointer: i32) -> Result<Vec<u8>> {
        if pointer < 0 {
            bail!("source returned error code {pointer}");
        }

        let memory = self.memory()?;
        let mut header = [0u8; 12];
        memory
            .read(&self.store, pointer as usize, &mut header)
            .context("failed to read source result header")?;

        let len = i32::from_le_bytes(header[0..4].try_into().unwrap());
        if len == -1 {
            let total_len = i32::from_le_bytes(header[8..12].try_into().unwrap());
            if total_len < 12 {
                self.free_result(pointer);
                bail!("source returned an invalid error message");
            }
            let message_len = (total_len - 12) as usize;
            let mut message = vec![0u8; message_len];
            memory
                .read(&self.store, pointer as usize + 12, &mut message)
                .context("failed to read source error message")?;
            self.free_result(pointer);
            bail!("{}", String::from_utf8_lossy(&message));
        }

        if len < 8 {
            self.free_result(pointer);
            bail!("source returned an invalid result length");
        }

        // `aidoku::imports::std::encode` prefixes the postcard payload with an
        // 8 byte header and stores the *total* length (header included), so the
        // payload itself is `len - 8` bytes long.
        let mut data = vec![0u8; (len - 8) as usize];
        memory
            .read(&self.store, pointer as usize + 8, &mut data)
            .context("failed to read source result")?;
        self.free_result(pointer);
        Ok(data)
    }

    fn free_result(&mut self, pointer: i32) {
        if let Ok(function) = self
            .instance
            .get_typed_func::<i32, ()>(&mut self.store, "free_result")
        {
            let _ = function.call(&mut self.store, pointer);
        }
    }

    fn invoke2<T: DeserializeOwned>(&mut self, name: &str, a: i32, b: i32) -> Result<T> {
        let function = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, name)
            .with_context(|| format!("source does not export {name}"))?;
        let pointer = function.call(&mut self.store, (a, b))?;
        let data = self.read_result(pointer)?;
        postcard::from_bytes(&data).context("failed to deserialize source result")
    }

    fn invoke3<T: DeserializeOwned>(&mut self, name: &str, a: i32, b: i32, c: i32) -> Result<T> {
        let function = self
            .instance
            .get_typed_func::<(i32, i32, i32), i32>(&mut self.store, name)
            .with_context(|| format!("source does not export {name}"))?;
        let pointer = function.call(&mut self.store, (a, b, c))?;
        let data = self.read_result(pointer)?;
        postcard::from_bytes(&data).context("failed to deserialize source result")
    }

    fn store_encoded<T: Serialize>(&mut self, value: &T) -> i32 {
        self.store.data_mut().store.store_encoded(value)
    }

    fn store_string(&mut self, value: String) -> i32 {
        self.store.data_mut().store.store_string(value)
    }

    fn remove_descriptor(&mut self, descriptor: i32) {
        self.store.data_mut().store.remove(descriptor);
    }

    fn has_export(&self, name: &str) -> bool {
        self.instance.get_export(&self.store, name).is_some()
    }

    pub fn search_mangas(
        &mut self,
        _cancellation_token: tokio_util::sync::CancellationToken,
        query: String,
    ) -> Result<Vec<Manga>> {
        let query_descriptor = self.store_string(query);
        let filters_descriptor = self.store_encoded(&Vec::<super::model::FilterValue>::new());

        let result = (|| {
            let page_result: super::model::MangaPageResult = self.invoke3(
                "get_search_manga_list",
                query_descriptor,
                1,
                filters_descriptor,
            )?;
            Ok::<_, anyhow::Error>(
                page_result
                    .entries
                    .into_iter()
                    .map(|manga| self.convert_manga(manga))
                    .collect(),
            )
        })();

        self.remove_descriptor(query_descriptor);
        self.remove_descriptor(filters_descriptor);
        result
    }

    pub fn get_manga_list(
        &mut self,
        _cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<Vec<Manga>> {
        // Used by source listings. rakuyomi's UI currently drives everything
        // through search, but supporting this makes the source API more complete.
        let listing = super::model::Listing {
            id: String::new(),
            name: String::new(),
            kind: super::model::ListingKind::Default,
        };
        let listing_descriptor = self.store_encoded(&listing);
        let result = (|| {
            let page_result: super::model::MangaPageResult =
                self.invoke2("get_manga_list", listing_descriptor, 1)?;
            Ok::<_, anyhow::Error>(
                page_result
                    .entries
                    .into_iter()
                    .map(|manga| self.convert_manga(manga))
                    .collect(),
            )
        })();
        self.remove_descriptor(listing_descriptor);
        result
    }

    pub fn get_chapter_list(
        &mut self,
        _cancellation_token: tokio_util::sync::CancellationToken,
        manga_id: String,
    ) -> Result<Vec<Chapter>> {
        let manga = AidokuManga {
            key: manga_id.clone(),
            ..Default::default()
        };
        let manga_descriptor = self.store_encoded(&manga);
        let result = (|| {
            let updated: AidokuManga = self.invoke3("get_manga_update", manga_descriptor, 0, 1)?;
            Ok::<_, anyhow::Error>(
                updated
                    .chapters
                    .unwrap_or_default()
                    .into_iter()
                    .enumerate()
                    .map(|(index, chapter)| self.convert_chapter(manga_id.clone(), index, chapter))
                    .collect(),
            )
        })();
        self.remove_descriptor(manga_descriptor);
        result
    }

    pub fn get_page_list(
        &mut self,
        _cancellation_token: tokio_util::sync::CancellationToken,
        manga_id: String,
        chapter_id: String,
        chapter_num: Option<f64>,
    ) -> Result<Vec<Page>> {
        let manga = AidokuManga {
            key: manga_id.clone(),
            ..Default::default()
        };
        let chapter = AidokuChapter {
            key: chapter_id.clone(),
            chapter_number: chapter_num.map(|value| value as f32),
            ..Default::default()
        };
        let manga_descriptor = self.store_encoded(&manga);
        let chapter_descriptor = self.store_encoded(&chapter);
        let result = (|| {
            let pages: Vec<super::model::Page> =
                self.invoke2("get_page_list", manga_descriptor, chapter_descriptor)?;
            pages
                .into_iter()
                .enumerate()
                .map(|(index, page)| {
                    self.convert_page(manga_id.clone(), chapter_id.clone(), index, page)
                })
                .collect()
        })();
        self.remove_descriptor(manga_descriptor);
        self.remove_descriptor(chapter_descriptor);
        result
    }

    pub fn get_image_request(
        &mut self,
        url: Url,
        context: Option<HashMap<String, String>>,
    ) -> Result<reqwest::Request> {
        if self.has_export("get_image_request") {
            let url_descriptor = self.store_encoded(&url.to_string());
            let context_descriptor = match context.as_ref() {
                Some(context) => self.store_encoded(context),
                None => -1,
            };

            let result = (|| {
                let request_descriptor: i32 =
                    self.invoke2("get_image_request", url_descriptor, context_descriptor)?;
                let item = self
                    .store
                    .data_mut()
                    .store
                    .take(request_descriptor)
                    .context("source returned an invalid image request")?;
                let request = match item {
                    StoreItem::Request(request) => *request,
                    _ => bail!("source returned an unexpected image request value"),
                };
                build_reqwest_request(request, &url)
            })();

            self.remove_descriptor(url_descriptor);
            if context_descriptor >= 0 {
                self.remove_descriptor(context_descriptor);
            }
            return result;
        }

        default_image_request(url)
    }

    fn convert_manga(&self, manga: AidokuManga) -> Manga {
        Manga {
            source_id: self.manifest.info.id.clone(),
            id: manga.key,
            title: if manga.title.is_empty() {
                None
            } else {
                Some(manga.title)
            },
            author: manga.authors.map(|authors| authors.join(", ")),
            artist: manga.artists.map(|artists| artists.join(", ")),
            description: manga.description,
            tags: manga.tags,
            cover_url: manga.cover.and_then(|url| Url::parse(&url).ok()),
            url: manga.url.and_then(|url| Url::parse(&url).ok()),
            status: match manga.status {
                MangaStatus::Unknown => PublishingStatus::Unknown,
                MangaStatus::Ongoing => PublishingStatus::Ongoing,
                MangaStatus::Completed => PublishingStatus::Completed,
                MangaStatus::Cancelled => PublishingStatus::Cancelled,
                MangaStatus::Hiatus => PublishingStatus::Hiatus,
            },
            nsfw: match manga.content_rating {
                ContentRating::Unknown | ContentRating::Safe => MangaContentRating::Safe,
                ContentRating::Suggestive => MangaContentRating::Suggestive,
                ContentRating::Nsfw => MangaContentRating::Nsfw,
            },
            viewer: match manga.viewer {
                Viewer::Unknown => MangaViewer::DefaultViewer,
                Viewer::LeftToRight => MangaViewer::Ltr,
                Viewer::RightToLeft => MangaViewer::Rtl,
                Viewer::Vertical => MangaViewer::Vertical,
                Viewer::Webtoon => MangaViewer::Scroll,
            },
            ..Default::default()
        }
    }

    fn convert_chapter(&self, manga_id: String, index: usize, chapter: AidokuChapter) -> Chapter {
        Chapter {
            source_id: self.manifest.info.id.clone(),
            id: chapter.key,
            manga_id,
            title: chapter.title.filter(|title| !title.is_empty()),
            scanlator: chapter.scanlators.map(|scanlators| scanlators.join(", ")),
            url: chapter.url.and_then(|url| Url::parse(&url).ok()),
            lang: chapter
                .language
                .filter(|language| !language.is_empty())
                .unwrap_or_else(|| "en".into()),
            chapter_num: chapter.chapter_number,
            volume_num: chapter.volume_number,
            date_uploaded: chapter.date_uploaded.and_then(|timestamp| {
                Utc.timestamp_opt(timestamp, 0)
                    .single()
                    .map(|date| date.with_timezone(&chrono_tz::UTC))
            }),
            source_order: index,
        }
    }

    fn convert_page(
        &self,
        manga_id: String,
        chapter_id: String,
        index: usize,
        page: super::model::Page,
    ) -> Result<Page> {
        let (image_url, text, context) = match page.content {
            PageContent::Url(url, context) => {
                let url = Url::parse(&url).context("source returned an invalid page URL")?;
                (Some(url), None, context)
            }
            PageContent::Text(text) => (None, Some(text), None),
            PageContent::Image(_) => {
                bail!("source returned an inline image, which is not supported yet")
            }
            PageContent::Zip(_, _) => {
                bail!("source returned a zip page, which is not supported yet")
            }
        };

        Ok(Page {
            source_id: self.manifest.info.id.clone(),
            chapter_id,
            index,
            image_url,
            base64: None,
            text,
            context,
            manga_id,
        })
    }
}

fn default_image_request(url: Url) -> Result<reqwest::Request> {
    let mut request = reqwest::Request::new(reqwest::Method::GET, url);
    request.headers_mut().insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(super::net::DEFAULT_USER_AGENT),
    );
    Ok(request)
}

fn build_reqwest_request(request: NetRequest, fallback_url: &Url) -> Result<reqwest::Request> {
    let url = request.url.unwrap_or_else(|| fallback_url.clone());
    let mut result = reqwest::Request::new(request.method.into_reqwest(), url);

    *result.headers_mut() = request.headers;
    if let Some(body) = request.body {
        *result.body_mut() = Some(body.into());
    }
    if let Some(timeout) = request.timeout {
        // reqwest::Request supports timeouts through the builder only on some
        // versions; keep the request default when it can't be set.
        let _ = timeout as Duration;
    }

    Ok(result)
}
