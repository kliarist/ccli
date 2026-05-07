//! Confluence Content API client — page comments.
//!
//! Locked decisions implemented:
//! - D-50: list_comments fetches body.storage + version for preview rendering
//! - D-51, D-52: add_comment accepts pre-wrapped storage XML (caller wraps plain text)
//!
//! Security:
//! - PAT NEVER appears in tracing output. #[instrument(skip(client))] on every async fn.
//! - All errors map to existing AppError variants (Auth/Network/Api) — no new variants.

use serde::Deserialize;
use tracing::instrument;

use crate::api::client::Client;
use crate::api::error::AppError;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub id: String,
    pub title: String,
    pub version: Option<CommentVersion>,
    pub body: Option<CommentBody>,
    #[serde(rename = "_links", default)]
    pub links: CommentLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CommentVersion {
    pub number: u32,
    pub when: Option<String>,
    pub by: Option<CommentAuthor>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CommentAuthor {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CommentBody {
    pub storage: Option<StorageBody>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct StorageBody {
    pub value: Option<String>,
    pub representation: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CommentLinks {
    pub webui: Option<String>,
    #[serde(rename = "self", default)]
    pub self_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentListResponse {
    pub results: Vec<Comment>,
}

/// GET /rest/api/content/{page_id}/child/comment?expand=body.storage,version&limit=50
/// Returns a Vec<Comment> in the order Confluence returns them.
/// Limit fixed at 50 for v1 (D-50; CONTEXT.md: comments don't paginate aggressively).
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn list_comments(client: &Client, page_id: &str) -> Result<Vec<Comment>, AppError> {
    let url = format!(
        "{}/rest/api/content/{}/child/comment",
        client.base_url().trim_end_matches('/'),
        page_id,
    );
    // WR-05: .query() builder percent-encodes all values
    let resp = client
        .inner()
        .get(&url)
        .query(&[
            ("expand", "body.storage,version"),
            ("limit", "50"),
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
            let parsed: CommentListResponse = resp.json().await.map_err(|e| {
                AppError::Api(format!("Failed to parse comment list: {}", e))
            })?;
            Ok(parsed.results)
        }
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} from /rest/api/content/{}/child/comment",
            status, page_id
        ))),
    }
}

/// POST /rest/api/content/{page_id}/child/comment
/// `xml_body` MUST be storage-XML (e.g. `<p>...</p>`); the caller wraps plain text per D-52.
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn add_comment(
    client: &Client,
    page_id: &str,
    xml_body: &str,
) -> Result<(), AppError> {
    let url = format!(
        "{}/rest/api/content/{}/child/comment",
        client.base_url().trim_end_matches('/'),
        page_id,
    );
    let body = serde_json::json!({
        "type": "comment",
        "body": {
            "storage": {
                "value": xml_body,
                "representation": "storage",
            }
        }
    });
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
        200 | 201 => Ok(()),
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} from POST /rest/api/content/{}/child/comment",
            status, page_id
        ))),
    }
}

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
    async fn list_comments_returns_vec_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/123/child/comment");
            then.status(200).json_body(serde_json::json!({
                "results": [{
                    "id": "1",
                    "title": "Re:p",
                    "version": {
                        "number": 1,
                        "when": "2026-05-01T00:00:00Z",
                        "by": {"displayName": "Alice"}
                    },
                    "body": {
                        "storage": {
                            "value": "<p>hi</p>",
                            "representation": "storage"
                        }
                    },
                    "_links": {}
                }],
                "start": 0,
                "limit": 50,
                "size": 1,
                "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let result = list_comments(&client, "123").await.expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "1");
    }

    #[tokio::test]
    async fn list_comments_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/123/child/comment");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = list_comments(&client, "123").await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn add_comment_posts_storage_xml_body() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST)
                .path("/rest/api/content/123/child/comment")
                .json_body_partial(r#"{"type":"comment"}"#);
            then.status(200);
        });
        let client = test_client(&server.base_url());
        let result = add_comment(&client, "123", "<p>hi</p>").await;
        assert!(result.is_ok());
        m.assert();
        // Also verify the xml body content was included
    }

    #[tokio::test]
    async fn add_comment_returns_auth_error_on_403() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/rest/api/content/123/child/comment");
            then.status(403);
        });
        let client = test_client(&server.base_url());
        let result = add_comment(&client, "123", "<p>hi</p>").await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }
}
