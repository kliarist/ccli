//! Confluence Content API client — pages and blog posts.
//!
//! Locked decisions implemented:
//! - D-33: list returned sorted by title ascending (sort happens in list_all_pages)
//! - D-37: ContentType enum drives type=page vs type=blogpost on every endpoint
//! - D-39: 409 on update returns AppError::Api with message containing "Conflict"
//! - D-43: PageDetail body included via expand=body.storage,version,ancestors
//! - Pitfall 1: update_page always sends current_version + 1 (caller passes current_version)
//! - Pitfall 2: update_page requires title arg even when unchanged
//! - Pitfall 5: create_page omits "ancestors" for ContentType::BlogPost
//! - Pitfall 6: pagination uses `_links.next` absence + `size < limit` (size is per-page)
//!
//! Security:
//! - PAT NEVER appears in tracing output. #[instrument(skip(client))] on every async fn.

use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::api::client::Client;
use crate::api::error::AppError;

/// Whether a Confluence content item is a page or a blog post.
///
/// `type=page` → space-tree page; `type=blogpost` → blog post (no parent page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Page,
    BlogPost,
}

impl ContentType {
    /// Confluence API string representation: "page" or "blogpost".
    pub fn as_api_str(&self) -> &'static str {
        match self {
            ContentType::Page => "page",
            ContentType::BlogPost => "blogpost",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ContentListResponse {
    pub results: Vec<Page>,
    pub start: u32,
    pub limit: u32,
    pub size: u32, // count on THIS page, not total — Pitfall 6
    #[serde(rename = "_links")]
    pub links: ContentListLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ContentListLinks {
    pub next: Option<String>,
    pub base: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Page {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub content_type: String, // "page" or "blogpost" — string from API, not the enum
    pub version: Option<PageVersion>,
    pub ancestors: Option<Vec<PageAncestor>>,
    #[serde(rename = "_links")]
    pub links: PageLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PageVersion {
    pub number: u32,
    pub when: Option<String>,
    pub by: Option<PageAuthor>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PageAuthor {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PageAncestor {
    pub id: String,
    pub title: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PageLinks {
    pub webui: Option<String>,
    #[serde(rename = "self")]
    pub self_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PageDetail {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub content_type: String,
    pub version: Option<PageVersion>,
    pub ancestors: Option<Vec<PageAncestor>>,
    pub body: Option<PageBody>,
    #[serde(rename = "_links")]
    pub links: PageLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PageBody {
    pub storage: Option<StorageBody>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct StorageBody {
    pub value: Option<String>,
    pub representation: Option<String>,
}
