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

use serde::Deserialize;
use tracing::{debug, instrument};

use crate::api::client::Client;
use crate::api::error::AppError;

/// Whether a Confluence content item is a page or a blog post.
///
/// `type=page` → space-tree page; `type=blogpost` → blog post (no parent page).
#[allow(dead_code)]
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

// ─── API functions ─────────────────────────────────────────────────────────────

/// Fetch all pages or blog posts in a space, walking all pagination pages.
///
/// Endpoint: GET /rest/api/content?spaceKey={K}&type={page|blogpost}&start={N}&limit=50
///                                 &expand=version,ancestors
/// Pagination: stop when `_links.next` is absent OR `size < limit` (Pitfall 6).
/// Result is sorted by title ascending (D-33).
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn list_all_pages(
    client: &Client,
    space_key: &str,
    content_type: ContentType,
) -> Result<Vec<Page>, AppError> {
    let mut all: Vec<Page> = Vec::new();
    let mut start = 0u32;
    let limit = 50u32;

    loop {
        // WR-05: use .query() builder so reqwest percent-encodes all values,
        // preventing query-string injection via crafted space keys (e.g. "DEV&type=blogpost").
        let base = format!(
            "{}/rest/api/content",
            client.base_url().trim_end_matches('/'),
        );
        debug!("Fetching content list page: GET {} start={}", base, start);

        let resp = client
            .inner()
            .get(&base)
            .query(&[
                ("spaceKey", space_key),
                ("type", content_type.as_api_str()),
                ("start", &start.to_string()),
                ("limit", &limit.to_string()),
                ("expand", "version,ancestors"),
            ])
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() || e.is_timeout() {
                    AppError::Network(format!("Cannot reach server: {}", e))
                } else {
                    AppError::Network(e.to_string())
                }
            })?;

        match resp.status().as_u16() {
            200 => {
                let page: ContentListResponse = resp
                    .json()
                    .await
                    .map_err(|e| AppError::Api(format!("Failed to parse content list: {}", e)))?;
                let fetched = page.results.len() as u32;
                let has_next = page.links.next.is_some();
                all.extend(page.results);
                if !has_next || fetched < limit {
                    break;
                }
                start += limit;
            }
            401 | 403 => {
                return Err(AppError::Auth(
                    "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
                ))
            }
            status => {
                return Err(AppError::Api(format!(
                    "Unexpected HTTP {} from /rest/api/content",
                    status
                )))
            }
        }
    }

    // D-33: sort by title ascending
    all.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(all)
}

/// Fetch a single page or blog post with body, version, and ancestors.
///
/// Endpoint: GET /rest/api/content/{id}?expand=body.storage,version,ancestors (D-43)
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn get_page_detail(client: &Client, page_id: &str) -> Result<PageDetail, AppError> {
    let url = format!(
        "{}/rest/api/content/{}?expand=body.storage,version,ancestors",
        client.base_url().trim_end_matches('/'),
        page_id,
    );
    debug!("Fetching page detail: GET {}", url);

    let resp = client.inner().get(&url).send().await.map_err(|e| {
        if e.is_connect() || e.is_timeout() {
            AppError::Network(format!("Cannot reach server: {}", e))
        } else {
            AppError::Network(e.to_string())
        }
    })?;

    match resp.status().as_u16() {
        200 => resp
            .json::<PageDetail>()
            .await
            .map_err(|e| AppError::Api(format!("Failed to parse page detail: {}", e))),
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} fetching page {}",
            status, page_id
        ))),
    }
}

/// Create a new page or blog post.
///
/// Endpoint: POST /rest/api/content
/// Body shape (D-37, D-40, Pitfall 5):
///   type: "page" | "blogpost"
///   title: required
///   space.key: required
///   ancestors: [{id}] — ONLY when content_type==Page AND parent_id is Some.
///                       Omitted entirely for blog posts (Pitfall 5).
///   body.storage.value: xml_content
///   body.storage.representation: "storage"
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn create_page(
    client: &Client,
    space_key: &str,
    title: &str,
    xml_content: &str,
    content_type: ContentType,
    parent_id: Option<&str>,
) -> Result<Page, AppError> {
    let url = format!(
        "{}/rest/api/content",
        client.base_url().trim_end_matches('/'),
    );

    // Build body object with conditional ancestors (Pitfall 5)
    let mut body = serde_json::json!({
        "type": content_type.as_api_str(),
        "title": title,
        "space": { "key": space_key },
        "body": {
            "storage": {
                "value": xml_content,
                "representation": "storage",
            }
        }
    });
    if matches!(content_type, ContentType::Page) {
        if let Some(pid) = parent_id {
            body["ancestors"] = serde_json::json!([{ "id": pid }]);
        }
    }
    debug!("Creating content: POST {}", url);

    let resp = client
        .inner()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::Network(format!("Cannot reach server: {}", e))
            } else {
                AppError::Network(e.to_string())
            }
        })?;

    match resp.status().as_u16() {
        200 | 201 => resp
            .json::<Page>()
            .await
            .map_err(|e| AppError::Api(format!("Failed to parse created page: {}", e))),
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} creating content",
            status
        ))),
    }
}

/// Update a page or blog post — increments version.
///
/// Endpoint: PUT /rest/api/content/{id}
/// Body (Pitfall 1, Pitfall 2):
///   id, type, title (required even unchanged), space.key,
///   version.number = current_version + 1,
///   body.storage.{value, representation:"storage"}
///
/// Errors:
/// - 409 → AppError::Api("Conflict: page was updated by someone else (version {N})")
/// - 401/403 → AppError::Auth
/// - other non-2xx → AppError::Api
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn update_page(
    client: &Client,
    page_id: &str,
    title: &str,
    space_key: &str,
    new_xml: &str,
    content_type: ContentType,
    current_version: u32,
) -> Result<(), AppError> {
    let url = format!(
        "{}/rest/api/content/{}",
        client.base_url().trim_end_matches('/'),
        page_id,
    );
    let body = serde_json::json!({
        "id": page_id,
        "type": content_type.as_api_str(),
        "title": title,
        "space": { "key": space_key },
        "version": { "number": current_version + 1 },
        "body": {
            "storage": {
                "value": new_xml,
                "representation": "storage",
            }
        }
    });
    debug!("Updating content: PUT {}", url);

    let resp = client
        .inner()
        .put(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::Network(format!("Cannot reach server: {}", e))
            } else {
                AppError::Network(e.to_string())
            }
        })?;

    match resp.status().as_u16() {
        200 | 204 => Ok(()),
        409 => Err(AppError::Api(format!(
            "Conflict: page was updated by someone else (version {}).",
            current_version
        ))),
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} updating page {}",
            status, page_id
        ))),
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use httpmock::prelude::*;

    fn test_client(base_url: &str) -> Client {
        Client::new(&Config {
            url: base_url.to_string(),
            token: "AT-test-token".to_string(),
        })
        .expect("client")
    }

    // ── list_all_pages ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_all_pages_returns_sorted_vec_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content")
                .query_param("type", "page")
                .query_param("spaceKey", "DEV");
            then.status(200).json_body(serde_json::json!({
                "results": [
                    {"id": "200", "title": "Zebra", "type": "page", "_links": {"webui": "/x/200"}},
                    {"id": "100", "title": "Alpha", "type": "page", "_links": {"webui": "/x/100"}},
                ],
                "start": 0, "limit": 50, "size": 2, "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let pages = list_all_pages(&client, "DEV", ContentType::Page)
            .await
            .expect("ok");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].title, "Alpha", "must be sorted by title ascending");
        assert_eq!(pages[1].title, "Zebra");
    }

    #[tokio::test]
    async fn list_all_pages_uses_blogpost_type_for_blog() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content")
                .query_param("type", "blogpost");
            then.status(200).json_body(serde_json::json!({
                "results": [{"id": "1", "title": "Post", "type": "blogpost", "_links": {}}],
                "start": 0, "limit": 50, "size": 1, "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let posts = list_all_pages(&client, "DEV", ContentType::BlogPost)
            .await
            .expect("ok");
        assert_eq!(posts.len(), 1);
    }

    #[tokio::test]
    async fn list_all_pages_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = list_all_pages(&client, "DEV", ContentType::Page).await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn list_all_pages_returns_auth_error_on_403() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content");
            then.status(403);
        });
        let client = test_client(&server.base_url());
        let result = list_all_pages(&client, "DEV", ContentType::Page).await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn list_all_pages_returns_api_error_on_500() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content");
            then.status(500);
        });
        let client = test_client(&server.base_url());
        let result = list_all_pages(&client, "DEV", ContentType::Page).await;
        assert!(matches!(result, Err(AppError::Api(_))));
    }

    #[tokio::test]
    async fn list_all_pages_paginates_until_no_next_link() {
        let server = MockServer::start();
        // First page: 50 results with _links.next set
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content")
                .query_param("start", "0");
            then.status(200).json_body(serde_json::json!({
                "results": (0u32..50).map(|i| serde_json::json!({
                    "id": i.to_string(),
                    "title": format!("Page {:03}", i),
                    "type": "page",
                    "_links": {"webui": format!("/x/{}", i)}
                })).collect::<Vec<_>>(),
                "start": 0, "limit": 50, "size": 50,
                "_links": {"next": "/rest/api/content?start=50&limit=50"}
            }));
        });
        // Second page: 2 results, no _links.next → stop
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content")
                .query_param("start", "50");
            then.status(200).json_body(serde_json::json!({
                "results": [
                    {"id": "50", "title": "Zebra A", "type": "page", "_links": {}},
                    {"id": "51", "title": "Zebra B", "type": "page", "_links": {}},
                ],
                "start": 50, "limit": 50, "size": 2,
                "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let pages = list_all_pages(&client, "DEV", ContentType::Page)
            .await
            .expect("ok");
        assert_eq!(pages.len(), 52, "must collect both pages");
        // Sorted: "Page 000" .. "Page 049" then "Zebra A", "Zebra B"
        assert_eq!(pages[0].title, "Page 000");
        assert_eq!(pages[51].title, "Zebra B");
    }

    // ── get_page_detail ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_page_detail_returns_detail_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content/123");
            then.status(200).json_body(serde_json::json!({
                "id": "123", "title": "Hello", "type": "page",
                "version": {"number": 5, "by": {"displayName": "alice"}},
                "body": {"storage": {"value": "<p>Hi</p>", "representation": "storage"}},
                "_links": {"webui": "/x/123"}
            }));
        });
        let client = test_client(&server.base_url());
        let d = get_page_detail(&client, "123").await.expect("ok");
        assert_eq!(d.title, "Hello");
        assert_eq!(d.version.unwrap().number, 5);
        assert_eq!(d.body.unwrap().storage.unwrap().value.unwrap(), "<p>Hi</p>");
    }

    #[tokio::test]
    async fn get_page_detail_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content/123");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = get_page_detail(&client, "123").await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn get_page_detail_returns_api_error_on_404() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content/NOTFOUND");
            then.status(404);
        });
        let client = test_client(&server.base_url());
        let result = get_page_detail(&client, "NOTFOUND").await;
        assert!(matches!(result, Err(AppError::Api(_))));
    }

    // ── create_page ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_page_with_parent_includes_ancestors_in_body() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST)
                .path("/rest/api/content")
                .json_body_partial(r#"{"type":"page","title":"New","ancestors":[{"id":"99"}]}"#);
            then.status(201).json_body(serde_json::json!({
                "id": "555", "title": "New", "type": "page", "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let p = create_page(
            &client,
            "DEV",
            "New",
            "<p>x</p>",
            ContentType::Page,
            Some("99"),
        )
        .await
        .expect("ok");
        assert_eq!(p.id, "555");
        m.assert();
    }

    #[tokio::test]
    async fn create_blog_post_omits_ancestors_field() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST).path("/rest/api/content").matches(|req| {
                let body: serde_json::Value =
                    serde_json::from_slice(req.body.as_deref().unwrap_or(&[]))
                        .unwrap_or(serde_json::Value::Null);
                body.get("type").and_then(|v| v.as_str()) == Some("blogpost")
                    && body.get("ancestors").is_none()
            });
            then.status(201).json_body(serde_json::json!({
                "id": "77", "title": "P", "type": "blogpost", "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        create_page(&client, "DEV", "P", "<p>x</p>", ContentType::BlogPost, None)
            .await
            .expect("ok");
        m.assert();
    }

    #[tokio::test]
    async fn create_page_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/rest/api/content");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result =
            create_page(&client, "DEV", "Title", "<p>x</p>", ContentType::Page, None).await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    // ── update_page ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn update_page_sends_version_number_plus_one() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(PUT)
                .path("/rest/api/content/42")
                .matches(|req| {
                    let body: serde_json::Value =
                        serde_json::from_slice(req.body.as_deref().unwrap_or(&[]))
                            .unwrap_or(serde_json::Value::Null);
                    body.get("version")
                        .and_then(|v| v.get("number"))
                        .and_then(|n| n.as_u64())
                        == Some(8)
                        && body.get("title").and_then(|v| v.as_str()) == Some("Title")
                });
            then.status(200).json_body(
                serde_json::json!({"id":"42","title":"Title","type":"page","_links":{}}),
            );
        });
        let client = test_client(&server.base_url());
        update_page(
            &client,
            "42",
            "Title",
            "DEV",
            "<p>updated</p>",
            ContentType::Page,
            7,
        )
        .await
        .expect("ok");
        m.assert();
    }

    #[tokio::test]
    async fn update_page_returns_conflict_message_on_409() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(PUT).path("/rest/api/content/42");
            then.status(409);
        });
        let client = test_client(&server.base_url());
        let result = update_page(&client, "42", "T", "DEV", "<p/>", ContentType::Page, 1).await;
        match result {
            Err(AppError::Api(msg)) => {
                assert!(msg.starts_with("Conflict:"), "got: {}", msg)
            }
            other => panic!("expected AppError::Api(\"Conflict: ...\"), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn update_page_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(PUT).path("/rest/api/content/42");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = update_page(&client, "42", "T", "DEV", "<p/>", ContentType::Page, 1).await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn update_page_returns_api_error_on_500() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(PUT).path("/rest/api/content/42");
            then.status(500);
        });
        let client = test_client(&server.base_url());
        let result = update_page(&client, "42", "T", "DEV", "<p/>", ContentType::Page, 1).await;
        assert!(matches!(result, Err(AppError::Api(_))));
    }
}
