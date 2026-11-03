//! Data structures used by the current Aidoku source ABI.
//!
//! These mirror the structures in `aidoku-rs` (`crates/lib/src/structs`). Postcard
//! serialization only cares about the order of fields and enum variants, so it is
//! important to keep this file in sync with the upstream definitions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type PageContext = HashMap<String, String>;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MangaStatus {
    #[default]
    Unknown = 0,
    Ongoing,
    Completed,
    Cancelled,
    Hiatus,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentRating {
    #[default]
    Unknown = 0,
    Safe,
    Suggestive,
    Nsfw,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Viewer {
    #[default]
    Unknown = 0,
    LeftToRight,
    RightToLeft,
    Vertical,
    Webtoon,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateStrategy {
    #[default]
    Always,
    Never,
}

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manga {
    pub key: String,
    pub title: String,
    pub cover: Option<String>,
    pub artists: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: MangaStatus,
    pub content_rating: ContentRating,
    pub viewer: Viewer,
    pub update_strategy: UpdateStrategy,
    pub next_update_time: Option<i64>,
    pub chapters: Option<Vec<Chapter>>,
}

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MangaPageResult {
    pub entries: Vec<Manga>,
    pub has_next_page: bool,
}

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub key: String,
    pub title: Option<String>,
    pub chapter_number: Option<f32>,
    pub volume_number: Option<f32>,
    pub date_uploaded: Option<i64>,
    pub scanlators: Option<Vec<String>>,
    pub url: Option<String>,
    pub language: Option<String>,
    pub thumbnail: Option<String>,
    pub locked: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum PageContent {
    Url(String, Option<PageContext>),
    Text(String),
    Image(i32),
    Zip(String, String),
}

impl Default for PageContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub content: PageContent,
    pub thumbnail: Option<String>,
    pub has_description: bool,
    pub description: Option<String>,
}

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    pub id: String,
    pub name: String,
    pub kind: ListingKind,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListingKind {
    #[default]
    Default,
    List,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FilterValue {
    Text {
        id: String,
        value: String,
    },
    Sort {
        id: String,
        index: i32,
        ascending: bool,
    },
    Check {
        id: String,
        value: i32,
    },
    Select {
        id: String,
        value: String,
    },
    MultiSelect {
        id: String,
        included: Vec<String>,
        excluded: Vec<String>,
    },
    Range {
        id: String,
        from: Option<f32>,
        to: Option<f32>,
    },
}
