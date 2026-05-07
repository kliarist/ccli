//! Confluence Space API client.
//!
//! Locked decisions implemented:
//! - D-14: pre-fetch all spaces on TUI launch (list_all_spaces)
//! - D-17: ccli space list fetches all pages (same walk as D-14)
//! - D-18: preview pane shows space metadata only (get_space_detail)
//! - D-19: 150ms debounce + session cache (types defined here; debounce logic in tui/app.rs)
//! - D-24: URL resolved from _links.webui, never hand-built
//!
//! Security:
//! - Token NEVER appears in tracing logs.
//! - #[instrument(skip(client))] applied to all public async functions.

use serde::Deserialize;
use tracing::{debug, instrument};

use crate::api::client::Client;
use crate::api::error::AppError;

// ─── Serde structs ────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceListResponse {
    pub results: Vec<Space>,
    pub start: u32,
    pub limit: u32,
    pub size: u32, // count of results on THIS page (not total) — Pitfall 7
    #[serde(rename = "_links")]
    pub links: SpaceListLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpaceListLinks {
    pub next: Option<String>, // present when more pages exist (A2)
    pub base: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Space {
    pub id: u64,
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub space_type: Option<String>, // "global", "personal", "archived"
    #[serde(rename = "_links")]
    pub links: SpaceLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpaceLinks {
    pub webui: Option<String>, // relative path e.g. "/display/DEV" — Pitfall 3
    #[serde(rename = "self")]
    pub self_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceDetail {
    pub id: u64,
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub space_type: Option<String>,
    pub description: Option<SpaceDescription>,
    pub homepage: Option<SpaceHomepage>,
    #[serde(rename = "_links")]
    pub links: SpaceLinks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpaceDescription {
    pub plain: Option<PlainBody>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PlainBody {
    pub value: Option<String>,
    pub representation: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceHomepage {
    pub id: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "_links")]
    pub links: Option<SpaceLinks>,
}

// ─── API functions ─────────────────────────────────────────────────────────────

/// Fetch all spaces from GET /rest/api/space, walking all pages.
///
/// Pagination: walks until `_links.next` is absent OR `size < limit`
/// (RESEARCH.md Pattern 5, Pitfall 7 — `size` is per-page count, not total).
/// Returns results sorted by key ascending (D-17, matches jira-cli and Confluence UI defaults).
#[instrument(skip(client))]
pub async fn list_all_spaces(client: &Client) -> Result<Vec<Space>, AppError> {
    let mut all: Vec<Space> = Vec::new();
    let mut start = 0u32;
    let limit = 25u32;

    loop {
        let url = format!(
            "{}/rest/api/space?start={}&limit={}",
            client.base_url().trim_end_matches('/'),
            start,
            limit
        );
        debug!("Fetching spaces page: GET {}", url);

        let resp = client.inner().get(&url).send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::Network(format!("Cannot reach server: {}", e))
            } else {
                AppError::Network(e.to_string())
            }
        })?;

        match resp.status().as_u16() {
            200 => {
                let page: SpaceListResponse = resp
                    .json()
                    .await
                    .map_err(|e| AppError::Api(format!("Failed to parse space list: {}", e)))?;
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
                    "Unexpected HTTP {} from /rest/api/space",
                    status
                )))
            }
        }
    }

    // D-17: sort by key ascending
    all.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(all)
}

/// Fetch detail for a single space: GET /rest/api/space/{key}?expand=description.plain,homepage
///
/// D-18: preview pane shows space metadata only (key, name, description, homepage title).
/// D-19: caller is responsible for 150ms debounce; result should be cached in preview_cache.
#[instrument(skip(client))]
pub async fn get_space_detail(client: &Client, key: &str) -> Result<SpaceDetail, AppError> {
    let url = format!(
        "{}/rest/api/space/{}?expand=description.plain,homepage",
        client.base_url().trim_end_matches('/'),
        key
    );
    debug!("Fetching space detail: GET {}", url);

    let resp = client.inner().get(&url).send().await.map_err(|e| {
        if e.is_connect() || e.is_timeout() {
            AppError::Network(format!("Cannot reach server: {}", e))
        } else {
            AppError::Network(e.to_string())
        }
    })?;

    match resp.status().as_u16() {
        200 => resp
            .json::<SpaceDetail>()
            .await
            .map_err(|e| AppError::Api(format!("Failed to parse space detail: {}", e))),
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} fetching space {}",
            status, key
        ))),
    }
}

/// Resolve the absolute browser URL for a space.
///
/// `_links.webui` is a RELATIVE path (e.g. "/display/DEV") — must be joined
/// onto base_url. Never hand-build from the space key (D-24, Pitfall 3).
pub fn resolve_webui_url(base_url: &str, webui: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), webui)
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

    #[tokio::test]
    async fn list_all_spaces_returns_sorted_vec_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space");
            then.status(200).json_body(serde_json::json!({
                "results": [
                    {"id": 2, "key": "ZZZ", "name": "Zebra", "_links": {"webui": "/display/ZZZ"}},
                    {"id": 1, "key": "AAA", "name": "Alpha", "_links": {"webui": "/display/AAA"}},
                ],
                "start": 0, "limit": 25, "size": 2,
                "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let spaces = list_all_spaces(&client).await.expect("ok");
        assert_eq!(spaces.len(), 2);
        assert_eq!(spaces[0].key, "AAA", "must be sorted by key ascending");
        assert_eq!(spaces[1].key, "ZZZ");
    }

    #[tokio::test]
    async fn list_all_spaces_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = list_all_spaces(&client).await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn list_all_spaces_returns_auth_error_on_403() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space");
            then.status(403);
        });
        let client = test_client(&server.base_url());
        let result = list_all_spaces(&client).await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn list_all_spaces_returns_api_error_on_500() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space");
            then.status(500);
        });
        let client = test_client(&server.base_url());
        let result = list_all_spaces(&client).await;
        assert!(matches!(result, Err(AppError::Api(_))));
    }

    #[tokio::test]
    async fn list_all_spaces_paginates_until_no_next_link() {
        let server = MockServer::start();
        // First page: 25 results, has _links.next
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/space")
                .query_param("start", "0");
            then.status(200).json_body(serde_json::json!({
                "results": (0..25u32).map(|i| serde_json::json!({
                    "id": i, "key": format!("SP{:02}", i), "name": format!("Space {}", i),
                    "_links": {"webui": format!("/display/SP{:02}", i)}
                })).collect::<Vec<_>>(),
                "start": 0, "limit": 25, "size": 25,
                "_links": {"next": "/rest/api/space?start=25&limit=25"}
            }));
        });
        // Second page: 2 results, no _links.next → stop
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/space")
                .query_param("start", "25");
            then.status(200).json_body(serde_json::json!({
                "results": [
                    {"id": 25, "key": "ZA0", "name": "Extra 1", "_links": {"webui": "/display/ZA0"}},
                    {"id": 26, "key": "ZA1", "name": "Extra 2", "_links": {"webui": "/display/ZA1"}},
                ],
                "start": 25, "limit": 25, "size": 2,
                "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let spaces = list_all_spaces(&client).await.expect("ok");
        assert_eq!(spaces.len(), 27, "must collect both pages");
        // Result must be sorted by key ascending
        assert_eq!(spaces[0].key, "SP00");
        assert_eq!(spaces[26].key, "ZA1");
    }

    #[tokio::test]
    async fn get_space_detail_returns_detail_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space/DEV");
            then.status(200).json_body(serde_json::json!({
                "id": 1,
                "key": "DEV",
                "name": "Development",
                "type": "global",
                "description": {
                    "plain": {"value": "The dev space", "representation": "plain"}
                },
                "homepage": {"id": "123", "title": "Home Page", "_links": {}},
                "_links": {"webui": "/display/DEV"}
            }));
        });
        let client = test_client(&server.base_url());
        let detail = get_space_detail(&client, "DEV").await.expect("ok");
        assert_eq!(detail.key, "DEV");
        assert_eq!(detail.name, "Development");
        assert_eq!(
            detail.description.unwrap().plain.unwrap().value.unwrap(),
            "The dev space"
        );
    }

    #[tokio::test]
    async fn get_space_detail_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space/DEV");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = get_space_detail(&client, "DEV").await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn get_space_detail_returns_api_error_on_404() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space/NOTFOUND");
            then.status(404);
        });
        let client = test_client(&server.base_url());
        let result = get_space_detail(&client, "NOTFOUND").await;
        assert!(matches!(result, Err(AppError::Api(_))));
    }

    #[tokio::test]
    async fn resolve_webui_url_joins_base_and_relative_path() {
        assert_eq!(
            resolve_webui_url("https://confluence.example.com", "/display/DEV"),
            "https://confluence.example.com/display/DEV"
        );
    }

    #[tokio::test]
    async fn resolve_webui_url_strips_trailing_slash_from_base() {
        assert_eq!(
            resolve_webui_url("https://confluence.example.com/", "/display/DEV"),
            "https://confluence.example.com/display/DEV"
        );
    }
}
