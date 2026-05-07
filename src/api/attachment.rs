//! Confluence Content API client — page attachments.
//!
//! Locked decisions implemented:
//! - ATTCH-01 / D-56: list_attachments returns metadata for table rendering
//! - ATTCH-02 / D-54: get_attachment returns raw bytes; caller writes to disk
//! - ATTCH-03 / D-55: add_attachment uses multipart upload with X-Atlassian-Token: no-check
//!
//! Security:
//! - PAT NEVER appears in tracing output. #[instrument(skip(client))] on every async fn.
//! - All errors map to existing AppError variants (Auth/Network/Api) — no new variants.

use std::path::Path;

use bytes::Bytes;
use reqwest::multipart;
use serde::Deserialize;
use tracing::instrument;

use crate::api::client::Client;
use crate::api::error::AppError;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub title: String,
    pub extensions: Option<AttachmentExtensions>,
    pub version: Option<AttachmentVersion>,
    #[serde(rename = "_links", default)]
    pub links: AttachmentLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct AttachmentExtensions {
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    #[serde(rename = "fileSize")]
    pub file_size: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct AttachmentVersion {
    pub when: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AttachmentLinks {
    pub download: Option<String>,
    pub webui: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AttachmentListResponse {
    pub results: Vec<Attachment>,
}

/// GET /rest/api/content/{page_id}/child/attachment?limit=50
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn list_attachments(client: &Client, page_id: &str) -> Result<Vec<Attachment>, AppError> {
    let url = format!(
        "{}/rest/api/content/{}/child/attachment",
        client.base_url().trim_end_matches('/'),
        page_id,
    );
    let resp = client
        .inner()
        .get(&url)
        .query(&[("limit", "50")])
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
            let parsed: AttachmentListResponse = resp.json().await.map_err(|e| {
                AppError::Api(format!("Failed to parse attachment list: {}", e))
            })?;
            Ok(parsed.results)
        }
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} from /rest/api/content/{}/child/attachment",
            status, page_id
        ))),
    }
}

/// Download an attachment by filename. Steps:
///   1. List attachments, find the one whose title matches `filename`
///   2. Resolve `_links.download` against base_url
///   3. GET the download URL, return the bytes
/// On 404 / not-found: AppError::Api with message including the filename.
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn get_attachment(
    client: &Client,
    page_id: &str,
    filename: &str,
) -> Result<Bytes, AppError> {
    let attachments = list_attachments(client, page_id).await?;
    let target = attachments.iter().find(|a| a.title == filename).ok_or_else(|| {
        AppError::Api(format!(
            "Attachment '{}' not found on page {}",
            filename, page_id
        ))
    })?;
    let download_path = target.links.download.as_deref().ok_or_else(|| {
        AppError::Api(format!("Attachment '{}' has no download link", filename))
    })?;
    // download path may be absolute path on the server (starts with /); resolve against base_url.
    let download_url = if download_path.starts_with("http") {
        download_path.to_string()
    } else {
        format!("{}{}", client.base_url().trim_end_matches('/'), download_path)
    };
    let resp = client
        .inner()
        .get(&download_url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Download failed: {}", e)))?;
    match resp.status().as_u16() {
        200 => resp
            .bytes()
            .await
            .map_err(|e| AppError::Network(format!("Download failed: {}", e))),
        401 | 403 => Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        )),
        status => Err(AppError::Api(format!(
            "Unexpected HTTP {} downloading attachment '{}'",
            status, filename
        ))),
    }
}

/// Upload a file as an attachment. MIME auto-detected from extension (D-55);
/// unknown extensions fall back to application/octet-stream.
/// Sets header `X-Atlassian-Token: no-check` (REQUIRED by Confluence DC).
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn add_attachment(
    client: &Client,
    page_id: &str,
    file_path: &Path,
) -> Result<(), AppError> {
    let file_bytes = std::fs::read(file_path)
        .map_err(|e| AppError::Api(format!("Cannot read file {:?}: {}", file_path, e)))?;
    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload.bin")
        .to_string();
    let mime_str = mime_guess::from_path(file_path)
        .first_or_octet_stream()
        .to_string();
    let part = multipart::Part::bytes(file_bytes)
        .file_name(filename.clone())
        .mime_str(&mime_str)
        .map_err(|e| AppError::Api(format!("Invalid MIME type: {}", e)))?;
    let form = multipart::Form::new().part("file", part);
    let url = format!(
        "{}/rest/api/content/{}/child/attachment",
        client.base_url().trim_end_matches('/'),
        page_id
    );
    let resp = client
        .inner()
        .post(&url)
        .header("X-Atlassian-Token", "no-check") // REQUIRED for DC attachment upload
        .multipart(form)
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
            "Unexpected HTTP {} from POST /rest/api/content/{}/child/attachment",
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
    async fn list_attachments_returns_vec_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/123/child/attachment");
            then.status(200).json_body(serde_json::json!({
                "results": [{
                    "id": "att1",
                    "title": "document.pdf",
                    "extensions": {
                        "mediaType": "application/pdf",
                        "fileSize": 1024
                    },
                    "version": {"when": "2026-05-01T00:00:00Z"},
                    "_links": {"download": "/download/attachments/123/document.pdf"}
                }],
                "start": 0,
                "limit": 50,
                "size": 1,
                "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let result = list_attachments(&client, "123").await.expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "document.pdf");
    }

    #[tokio::test]
    async fn list_attachments_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/123/child/attachment");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = list_attachments(&client, "123").await;
        assert!(matches!(result, Err(AppError::Auth(_))));
    }

    #[tokio::test]
    async fn get_attachment_downloads_bytes() {
        let server = MockServer::start();
        // Mock the listing endpoint
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/123/child/attachment");
            then.status(200).json_body(serde_json::json!({
                "results": [{
                    "id": "att1",
                    "title": "test.bin",
                    "extensions": null,
                    "version": null,
                    "_links": {"download": "/download/attachments/123/test.bin"}
                }],
                "start": 0,
                "limit": 50,
                "size": 1,
                "_links": {}
            }));
        });
        // Mock the download endpoint
        server.mock(|when, then| {
            when.method(GET)
                .path("/download/attachments/123/test.bin");
            then.status(200).body(vec![1u8, 2, 3, 4]);
        });
        let client = test_client(&server.base_url());
        let result = get_attachment(&client, "123", "test.bin").await.expect("ok");
        assert_eq!(result.as_ref(), &[1u8, 2, 3, 4]);
    }

    #[tokio::test]
    async fn get_attachment_returns_api_error_when_not_found() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/123/child/attachment");
            then.status(200).json_body(serde_json::json!({
                "results": [],
                "start": 0,
                "limit": 50,
                "size": 0,
                "_links": {}
            }));
        });
        let client = test_client(&server.base_url());
        let result = get_attachment(&client, "123", "missing.pdf").await;
        match result {
            Err(AppError::Api(msg)) => {
                assert!(msg.contains("missing.pdf"), "error should contain filename: {}", msg);
            }
            other => panic!("expected AppError::Api, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn add_attachment_sends_multipart_with_x_atlassian_token_header() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST)
                .path("/rest/api/content/123/child/attachment")
                .header("X-Atlassian-Token", "no-check");
            then.status(200);
        });
        // Create a temp file to upload
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        std::fs::write(tmp.path(), b"test content").expect("write");
        let client = test_client(&server.base_url());
        let result = add_attachment(&client, "123", tmp.path()).await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        m.assert();
    }
}
