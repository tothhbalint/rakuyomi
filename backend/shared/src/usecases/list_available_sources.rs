use anyhow::{Context, Result};
use futures::{stream, StreamExt, TryStreamExt};
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::model::{SourceId, SourceInformation};

#[derive(Debug, Deserialize)]
pub struct SourceListItem {
    pub id: SourceId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: usize,
    #[serde(default, alias = "downloadURL", alias = "download_url", alias = "file")]
    pub download_url: Option<String>,
}

pub async fn fetch_source_list(url: &Url) -> Result<Vec<SourceListItem>> {
    let response = reqwest::get(url.clone())
        .await
        .with_context(|| format!("failed to fetch source list at {url}"))?;

    let value = response
        .json::<Value>()
        .await
        .with_context(|| format!("failed to parse source list at {url}"))?;

    parse_source_list(&value).with_context(|| format!("invalid source list at {url}"))
}

pub fn parse_source_list(value: &Value) -> Result<Vec<SourceListItem>> {
    // Current source lists use an object with a `sources` array:
    //   { "name": "...", "sources": [ { "id": "...", "downloadURL": "..." } ] }
    // Older lists were just an array of items.
    let items = value
        .get("sources")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .context("source list has no `sources` array")?;

    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let item: SourceListItem = serde_json::from_value(item.clone())?;
        if item.download_url.is_some() {
            result.push(item);
        }
    }
    Ok(result)
}

pub async fn list_available_sources(source_lists: Vec<Url>) -> Result<Vec<SourceInformation>> {
    let source_informations = stream::iter(source_lists)
        .then(|source_list| async move {
            let items = fetch_source_list(&source_list).await?;
            anyhow::Ok(
                items
                    .into_iter()
                    .map(|item| SourceInformation {
                        id: item.id,
                        name: item.name.unwrap_or_else(|| "Unknown source".into()),
                        version: item.version,
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    // Multiple source lists commonly carry the same source. Keep the newest
    // version so the user sees (and installs) the most up-to-date source.
    let mut by_id: std::collections::HashMap<_, SourceInformation> =
        std::collections::HashMap::new();
    for source in source_informations {
        match by_id.get(&source.id) {
            Some(existing) if existing.version >= source.version => {}
            _ => {
                by_id.insert(source.id.clone(), source);
            }
        }
    }

    let mut source_informations = by_id.into_values().collect::<Vec<_>>();
    source_informations.sort_by_key(|source| source.name.clone());

    Ok(source_informations)
}
