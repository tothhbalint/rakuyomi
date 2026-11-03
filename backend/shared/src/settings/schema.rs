use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use regex::Regex;
use schemars::JsonSchema;
use serde::{
    de::{Unexpected, Visitor},
    Deserialize, Serialize,
};
use size::{Base, Size};
use url::Url;

#[derive(Clone, Debug, PartialEq)]
pub struct StorageSizeLimit(pub Size);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SourceSettingValue {
    Bool(bool),
    String(String),
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChapterSortingMode {
    ChapterAscending,
    #[default]
    ChapterDescending,
}

/// Settings used to configure rakuyomi's behavior.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct Settings {
    /// A list of URLs containing Aidoku-compatible source lists, which will be available
    /// for installation from inside the plugin.
    ///
    /// When the setting is omitted entirely, the current community list and the
    /// Yomu list are used. Set it to an empty array to disable source lists.
    #[serde(
        default = "default_source_lists",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub source_lists: Vec<Url>,

    /// If set, only chapters translated to those languages will be shown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,

    /// The size of the storage used to store download chapters. Defaults to 2 GB.
    /// Should be in the format: [positive real number] [GB|MB].
    #[serde(
        default = "default_storage_size_limit",
        skip_serializing_if = "is_default_storage_size_limit"
    )]
    pub storage_size_limit: StorageSizeLimit,

    /// The path to the folder where downloaded chapters will be stored.
    /// If not set, the default downloads folder will be used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<PathBuf>,

    /// Source-specific settings.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub source_settings: HashMap<String, HashMap<String, SourceSettingValue>>,

    /// The order in which chapters will be displayed in the chapter listing. Defaults to
    /// `volume_descending`.
    #[serde(default)]
    pub chapter_sorting_mode: ChapterSortingMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source_lists: default_source_lists(),
            languages: Vec::new(),
            storage_size_limit: default_storage_size_limit(),
            storage_path: None,
            source_settings: HashMap::new(),
            chapter_sorting_mode: ChapterSortingMode::default(),
        }
    }
}

/// Source lists used when `source_lists` is not present in `settings.json`.
///
/// These are the lists maintained by the Aidoku community and by Yomu. Both
/// are needed in practice: the community list has the widest catalogue while
/// the Yomu list carries sources that are not in the community list (and its
/// Kagane source works well with rakuyomi).
pub const DEFAULT_SOURCE_LISTS: [&str; 2] = [
    "https://aidoku-community.github.io/sources/index.min.json",
    "https://smexhy.github.io/yomu-aidoku-sources/index.min.json",
];

fn default_source_lists() -> Vec<Url> {
    DEFAULT_SOURCE_LISTS
        .iter()
        .map(|url| Url::parse(url).expect("built-in source list URL must be valid"))
        .collect()
}

fn default_storage_size_limit() -> StorageSizeLimit {
    StorageSizeLimit(Size::from_megabytes(2000))
}

fn is_default_storage_size_limit(size: &StorageSizeLimit) -> bool {
    *size == default_storage_size_limit()
}

impl Default for StorageSizeLimit {
    fn default() -> Self {
        Self(Size::from_bytes(0))
    }
}

impl Serialize for StorageSizeLimit {
    fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.format().with_base(Base::Base10).to_string())
    }
}

const STORAGE_SIZE_LIMIT_REGEX: &str = r"(?<value>[\d.]+) *(?<dimension>GB|MB)";

impl<'de> Deserialize<'de> for StorageSizeLimit {
    fn deserialize<D>(deserializer: D) -> std::prelude::v1::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as SerdeDeserialziationError;

        struct StorageSizeLimitVisitor;

        impl Visitor<'_> for StorageSizeLimitVisitor {
            type Value = StorageSizeLimit;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid size with dimensions (e.g. 2 GB, 2048 MB)")
            }

            fn visit_str<E>(self, v: &str) -> std::prelude::v1::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // FIXME this might be supported by `Size` eventually, but for now we just use a regex
                let regex = Regex::new(STORAGE_SIZE_LIMIT_REGEX).unwrap();
                let capture = regex.captures(v).ok_or_else(|| {
                    SerdeDeserialziationError::invalid_value(
                        Unexpected::Str(v),
                        &"a valid size with dimensions (e.g. 2 GB, 2048 MB)",
                    )
                })?;

                let value: f64 = capture["value"].parse().map_err(|_| {
                    SerdeDeserialziationError::invalid_value(
                        Unexpected::Str(v),
                        &"a valid float value as the size",
                    )
                })?;
                let dimension = &capture["dimension"];

                let size = match dimension {
                    "GB" => Size::from_gigabytes(value),
                    "MB" => Size::from_megabytes(value),
                    _ => panic!("unexpected dimension: {dimension}"),
                };

                Ok(StorageSizeLimit(size))
            }
        }

        deserializer.deserialize_str(StorageSizeLimitVisitor {})
    }
}

impl JsonSchema for StorageSizeLimit {
    fn schema_name() -> String {
        "StorageSizeLimit".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::StorageSizeLimit"))
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema_object = gen.subschema_for::<String>().into_object();

        schema_object.string().pattern = Some(STORAGE_SIZE_LIMIT_REGEX.to_owned());

        schema_object.into()
    }
}
