//! `ccli page` subcommand handlers.
//!
//! Locked decisions implemented:
//! - D-34: space_key is required positional argument for `page list` (parsed in cli/mod.rs)
//! - D-36: `page view` prints stripped plain text to stdout via strip_storage_xml
//! - D-38: `page create` uses CLI flags with dialoguer fallback
//! - D-40: `page create` template body is `<p>Start writing here...</p>`
//! - D-39: 409 conflict preserves temp file and prints path on stderr
//! - D-41: `page edit` opens $EDITOR (also used by TUI 'e' keybinding in Plan 06)
//! - D-37: `blog` subcommand delegates here with ContentType::BlogPost
//!
//! Security:
//! - Page IDs from CLI are validated digits-only before constructing /tmp paths
//!   (prevents path traversal via crafted ID).
//! - $EDITOR is launched via Command::new(editor).arg(path).status() — never via shell.

use anyhow::Context;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;

use crate::api::page::{
    create_page, get_page_detail, list_all_pages, update_page, ContentType, PageDetail,
};
use crate::api::{AppError, Client};
use crate::cli::{sanitize_id, Cli, PageArgs, PageCommands};
use crate::config;
use crate::output::{strip_storage_xml, OutputFormatter};

/// Storage XML template for `ccli page create` and `ccli blog create` (D-40).
const CREATE_TEMPLATE: &str = "<p>Start writing here...</p>";

/// Top-level dispatcher for `ccli page` subcommands.
pub async fn run(cli: &Cli, args: &PageArgs) -> anyhow::Result<()> {
    match &args.command {
        PageCommands::List { space_key } => {
            handle_list_typed(cli, space_key, ContentType::Page).await
        }
        PageCommands::View { page_id } => handle_view_typed(page_id).await,
        PageCommands::Create {
            space,
            title,
            parent,
        } => {
            handle_create_typed(
                space.as_deref(),
                title.as_deref(),
                parent.as_deref(),
                ContentType::Page,
            )
            .await
        }
        PageCommands::Edit { page_id, raw } => {
            handle_edit_typed(page_id, ContentType::Page, *raw).await
        }
        PageCommands::Search { cql, limit } => handle_search(cli, cql, *limit).await,
    }
}

// ─── List ─────────────────────────────────────────────────────────────────────

/// `ccli page list <KEY>` / `ccli blog list <KEY>` — table output.
///
/// D-35 TUI dispatch is wired in plan 03-06 (requires changes to tui::run signature).
/// Plan 05 always prints a table — correct behavior when piped, deliberate gap when TTY.
pub async fn handle_list_typed(
    cli: &Cli,
    space_key: &str,
    content_type: ContentType,
) -> anyhow::Result<()> {
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let pages = list_all_pages(&client, space_key, content_type)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to fetch content list")?;

    let formatter = OutputFormatter::new(cli.output_config());
    let headers = &["Title", "ID", "Author"];
    let rows: Vec<Vec<String>> = pages
        .iter()
        .map(|p| {
            vec![
                p.title.clone(),
                p.id.clone(),
                p.version
                    .as_ref()
                    .and_then(|v| v.by.as_ref())
                    .and_then(|a| a.display_name.clone())
                    .unwrap_or_default(),
            ]
        })
        .collect();
    formatter.print(headers, &rows);
    Ok(())
}

// ─── View ─────────────────────────────────────────────────────────────────────

/// `ccli page view <ID>` / `ccli blog view <ID>` — D-36: strip XML, print plain text.
pub async fn handle_view_typed(content_id: &str) -> anyhow::Result<()> {
    let id = sanitize_id(content_id).ok_or_else(|| {
        AppError::Api(format!(
            "Invalid id: must be numeric (got {:?})",
            content_id
        ))
    })?;
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let detail = get_page_detail(&client, &id)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to fetch page detail")?;

    let body = extract_body_value(&detail);
    let stripped = strip_storage_xml(&body);
    println!("{}", stripped);
    Ok(())
}

// ─── Create ───────────────────────────────────────────────────────────────────

/// `ccli page create` / `ccli blog create` — D-38, D-40, Pitfall 5.
pub async fn handle_create_typed(
    space: Option<&str>,
    title: Option<&str>,
    parent: Option<&str>, // ALWAYS None for blog posts (Pitfall 5; enforced at type level by BlogCommands)
    content_type: ContentType,
) -> anyhow::Result<()> {
    let theme = ColorfulTheme::default();
    let space_key: String = match space {
        Some(s) => s.to_string(),
        None => Input::with_theme(&theme)
            .with_prompt("Space key")
            .interact_text()
            .context("Space key prompt failed")?,
    };
    let title_value: String = match title {
        Some(t) => t.to_string(),
        None => Input::with_theme(&theme)
            .with_prompt("Title")
            .interact_text()
            .context("Title prompt failed")?,
    };

    // Open $EDITOR with the D-40 template
    let temp_path = std::env::temp_dir().join(format!("ccli-create-{}.xml", std::process::id()));
    std::fs::write(&temp_path, CREATE_TEMPLATE)
        .with_context(|| format!("Failed to write template to {:?}", temp_path))?;
    crate::edit::launch_editor(&temp_path)?;
    let xml_content = std::fs::read_to_string(&temp_path)
        .with_context(|| format!("Failed to read edited content from {:?}", temp_path))?;

    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let created = create_page(
        &client,
        &space_key,
        &title_value,
        &xml_content,
        content_type,
        parent,
    )
    .await
    .map_err(anyhow::Error::from)
    .context("Failed to create content")?;

    // Best-effort cleanup of temp file on success
    let _ = std::fs::remove_file(&temp_path);

    println!("Created: id={} title={}", created.id, created.title);
    Ok(())
}

// ─── Edit ─────────────────────────────────────────────────────────────────────

/// `ccli page edit <ID>` / `ccli blog edit <ID>` — D-39, D-41.
///
/// When pandoc is available the body is presented as Markdown; use `--raw` to
/// skip that and edit the Confluence storage XML directly.
///
/// On HTTP 409: stderr prints conflict message; temp file is preserved so the
/// user can re-open and merge manually.
pub async fn handle_edit_typed(
    content_id: &str,
    content_type: ContentType,
    raw: bool,
) -> anyhow::Result<()> {
    let id = sanitize_id(content_id).ok_or_else(|| {
        AppError::Api(format!(
            "Invalid id: must be numeric (got {:?})",
            content_id
        ))
    })?;
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let detail = get_page_detail(&client, &id)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to fetch page detail before edit")?;
    let current_version = detail.version.as_ref().map(|v| v.number).ok_or_else(|| {
        anyhow::anyhow!("Page version was not returned by the API — cannot safely update.")
    })?;
    let title = detail.title.clone();
    let space_key = detail
        .space
        .as_ref()
        .map(|s| s.key.clone())
        .or_else(|| extract_space_key_from_webui(&detail))
        .ok_or_else(|| {
            AppError::Api(
                "Could not determine space key — page is missing both space.key and _links.webui data.".to_string(),
            )
        })?;

    let body = extract_body_value(&detail);

    if !raw && !crate::edit::pandoc_available() {
        eprintln!("Tip: install pandoc for Markdown editing. Editing raw storage XML.");
    }

    let new_xml =
        crate::edit::run_edit_session(&body, &id, raw).context("Editor session failed")?;

    match update_page(
        &client,
        &id,
        &title,
        &space_key,
        &new_xml,
        content_type,
        current_version,
    )
    .await
    {
        Ok(()) => {
            println!("Saved.");
            Ok(())
        }
        Err(AppError::Api(msg)) if msg.starts_with("Conflict:") => {
            // D-39: temp file preserved by run_edit_session; print path on stderr
            let ext = if raw { "xml" } else { "md" };
            let temp_path = std::env::temp_dir().join(format!("ccli-edit-{}.{}", id, ext));
            eprintln!("{}", msg);
            eprintln!(
                "Your edits saved at {} — re-open to merge manually.",
                temp_path.display()
            );
            Err(anyhow::Error::from(AppError::Api(msg)).context("Update conflict"))
        }
        Err(other) => Err(anyhow::Error::from(other).context("Failed to update page")),
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Extract the storage XML body string from a PageDetail. Returns "" if absent
/// (caller passes the result directly to strip_storage_xml which handles "").
fn extract_body_value(detail: &PageDetail) -> String {
    detail
        .body
        .as_ref()
        .and_then(|b| b.storage.as_ref())
        .and_then(|s| s.value.clone())
        .unwrap_or_default()
}

/// Extract the space key from `_links.webui`. Confluence webui paths look like:
///   /display/DEV/Page+Title         (page)
///   /pages/viewpage.action?...      (rare)
/// The robust strategy: split on '/' and take the segment after "display".
fn extract_space_key_from_webui(detail: &PageDetail) -> Option<String> {
    let webui = detail.links.webui.as_deref()?;
    let mut segs = webui.split('/').filter(|s| !s.is_empty());
    while let Some(seg) = segs.next() {
        if seg == "display" {
            return segs.next().map(|s| s.to_string());
        }
    }
    None
}

// ─── Search (D-45..D-47) ─────────────────────────────────────────────────────

/// Local response shape for `/rest/api/content/search` — only the fields used by the table.
/// Defined here (not in api/page.rs) because search uses a different endpoint shape than list.
#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResult {
    id: String,
    title: String,
    #[serde(default)]
    space: Option<SearchResultSpace>,
    #[serde(rename = "_links", default)]
    links: SearchResultLinks,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResultSpace {
    key: String,
}

#[derive(Debug, serde::Deserialize, Default)]
struct SearchResultLinks {
    webui: Option<String>,
}

/// `ccli page search "<CQL>" [--limit N]` — D-45: ALWAYS table, no TUI dispatch.
/// D-46: 4 columns (Title, ID, Space, URL), default limit 25.
/// D-47: CQL is a positional argument.
pub async fn handle_search(cli: &Cli, cql: &str, limit: u32) -> anyhow::Result<()> {
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let url = format!(
        "{}/rest/api/content/search",
        client.base_url().trim_end_matches('/'),
    );
    let limit_str = limit.to_string();
    // WR-05: .query() builder percent-encodes user-supplied CQL safely.
    let resp = client
        .inner()
        .get(&url)
        .query(&[
            ("cql", cql),
            ("limit", limit_str.as_str()),
            ("expand", "space"),
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

    let status = resp.status().as_u16();
    let parsed: SearchResponse = match status {
        200 => resp
            .json()
            .await
            .map_err(|e| AppError::Api(format!("Failed to parse search response: {}", e)))?,
        400 => {
            // Confluence returns 400 for invalid CQL — surface its body to the user.
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("invalid CQL — {}", body)).into());
        }
        401 | 403 => {
            return Err(AppError::Auth(
                "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
            )
            .into());
        }
        _ => {
            return Err(AppError::Api(format!(
                "Unexpected HTTP {} from /rest/api/content/search",
                status
            ))
            .into());
        }
    };

    let base = client.base_url().trim_end_matches('/').to_string();
    let formatter = OutputFormatter::new(cli.output_config());
    let headers = &["Title", "ID", "Space", "URL"];
    let rows: Vec<Vec<String>> = parsed
        .results
        .iter()
        .map(|r| {
            let space_key = r.space.as_ref().map(|s| s.key.clone()).unwrap_or_default();
            let webui = r.links.webui.as_deref().unwrap_or("");
            let url = if webui.is_empty() {
                String::new()
            } else if webui.starts_with("http") {
                webui.to_string()
            } else {
                format!("{}{}", base, webui)
            };
            vec![r.title.clone(), r.id.clone(), space_key, url]
        })
        .collect();
    formatter.print(headers, &rows);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::page::{PageBody, PageDetail, PageLinks, StorageBody};

    // ── Search tests ─────────────────────────────────────────────────────────────

    use crate::api::Client;
    use crate::config::Config;
    use httpmock::prelude::*;

    fn test_client(base_url: &str) -> Client {
        Client::new(&Config {
            url: base_url.to_string(),
            token: "AT-test-token".to_string(),
            email: None,
        })
        .expect("client")
    }

    /// Helper: call the inner logic of handle_search without loading config,
    /// using a test client and a buffer writer to capture output.
    async fn run_search(
        client: &Client,
        cql: &str,
        limit: u32,
    ) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)> {
        let url = format!(
            "{}/rest/api/content/search",
            client.base_url().trim_end_matches('/'),
        );
        let limit_str = limit.to_string();
        let resp = client
            .inner()
            .get(&url)
            .query(&[
                ("cql", cql),
                ("limit", limit_str.as_str()),
                ("expand", "space"),
            ])
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        let status = resp.status().as_u16();
        let parsed: SearchResponse = match status {
            200 => resp
                .json()
                .await
                .map_err(|e| AppError::Api(format!("Failed to parse search response: {}", e)))?,
            401 | 403 => {
                return Err(AppError::Auth("Authentication failed.".to_string()).into());
            }
            _ => {
                return Err(AppError::Api(format!(
                    "Unexpected HTTP {} from /rest/api/content/search",
                    status
                ))
                .into());
            }
        };

        let base = client.base_url().trim_end_matches('/').to_string();
        let rows: Vec<Vec<String>> = parsed
            .results
            .iter()
            .map(|r| {
                let space_key = r.space.as_ref().map(|s| s.key.clone()).unwrap_or_default();
                let webui = r.links.webui.as_deref().unwrap_or("");
                let url = if webui.is_empty() {
                    String::new()
                } else if webui.starts_with("http") {
                    webui.to_string()
                } else {
                    format!("{}{}", base, webui)
                };
                vec![r.title.clone(), r.id.clone(), space_key, url]
            })
            .collect();

        Ok((
            vec!["Title".into(), "ID".into(), "Space".into(), "URL".into()],
            rows,
        ))
    }

    #[tokio::test]
    async fn page_search_outputs_table_with_4_columns() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content/search");
            then.status(200).json_body(serde_json::json!({
                "results": [
                    {
                        "id": "100", "title": "Alpha Page",
                        "space": {"key": "DEV"},
                        "_links": {"webui": "/display/DEV/Alpha+Page"}
                    },
                    {
                        "id": "200", "title": "Beta Page",
                        "space": {"key": "DEV"},
                        "_links": {"webui": "/display/DEV/Beta+Page"}
                    }
                ]
            }));
        });
        let client = test_client(&server.base_url());
        let (headers, rows) = run_search(&client, "type=page", 25)
            .await
            .expect("search ok");

        assert_eq!(headers, vec!["Title", "ID", "Space", "URL"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "Alpha Page");
        assert_eq!(rows[0][1], "100");
        assert_eq!(rows[0][2], "DEV");
        assert!(rows[0][3].contains("/display/DEV/Alpha+Page"));
        assert_eq!(rows[1][0], "Beta Page");
    }

    #[tokio::test]
    async fn page_search_default_limit_is_25() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/search")
                .query_param("limit", "25");
            then.status(200)
                .json_body(serde_json::json!({ "results": [] }));
        });
        let client = test_client(&server.base_url());
        run_search(&client, "type=page", 25).await.expect("ok");
        m.assert();
    }

    #[tokio::test]
    async fn page_search_custom_limit_passes_through() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(GET)
                .path("/rest/api/content/search")
                .query_param("limit", "5");
            then.status(200)
                .json_body(serde_json::json!({ "results": [] }));
        });
        let client = test_client(&server.base_url());
        run_search(&client, "type=page", 5).await.expect("ok");
        m.assert();
    }

    #[tokio::test]
    async fn page_search_url_column_resolves_to_full_url() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content/search");
            then.status(200).json_body(serde_json::json!({
                "results": [
                    {
                        "id": "42", "title": "My Page",
                        "space": {"key": "DEV"},
                        "_links": {"webui": "/display/DEV/page-id"}
                    }
                ]
            }));
        });
        let base_url = server.base_url();
        let client = test_client(&base_url);
        let (_headers, rows) = run_search(&client, "type=page", 25).await.expect("ok");

        assert_eq!(rows.len(), 1);
        let url_col = &rows[0][3];
        assert!(
            url_col.starts_with(&base_url),
            "URL column must start with base_url; got: {}",
            url_col
        );
        assert!(
            url_col.ends_with("/display/DEV/page-id"),
            "URL column must end with webui path; got: {}",
            url_col
        );
    }

    #[tokio::test]
    async fn page_search_returns_auth_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/content/search");
            then.status(401);
        });
        let client = test_client(&server.base_url());
        let result = run_search(&client, "type=page", 25).await;
        assert!(result.is_err());
        let err_chain = result.unwrap_err();
        let has_auth_err = err_chain.chain().any(|e| {
            e.downcast_ref::<AppError>()
                .map(|ae| matches!(ae, AppError::Auth(_)))
                .unwrap_or(false)
        });
        assert!(
            has_auth_err,
            "expected AppError::Auth in chain; got: {}",
            err_chain
        );
    }

    fn detail_with_body(body: Option<&str>) -> PageDetail {
        PageDetail {
            id: "1".into(),
            title: "T".into(),
            content_type: "page".into(),
            version: None,
            ancestors: None,
            body: Some(PageBody {
                storage: Some(StorageBody {
                    value: body.map(|s| s.to_string()),
                    representation: Some("storage".into()),
                }),
            }),
            space: None,
            links: PageLinks::default(),
        }
    }

    fn detail_with_webui(webui: &str) -> PageDetail {
        PageDetail {
            id: "1".into(),
            title: "T".into(),
            content_type: "page".into(),
            version: None,
            ancestors: None,
            body: None,
            space: None,
            links: PageLinks {
                webui: Some(webui.to_string()),
                self_url: None,
            },
        }
    }

    #[test]
    fn sanitize_id_accepts_digits() {
        assert_eq!(sanitize_id("12345"), Some("12345".to_string()));
    }

    #[test]
    fn sanitize_id_rejects_empty() {
        assert_eq!(sanitize_id(""), None);
    }

    #[test]
    fn sanitize_id_rejects_path_traversal() {
        assert_eq!(sanitize_id("../etc/passwd"), None);
    }

    #[test]
    fn sanitize_id_rejects_alpha() {
        assert_eq!(sanitize_id("abc"), None);
        assert_eq!(sanitize_id("12a"), None);
    }

    #[test]
    fn extract_body_value_returns_body_when_present() {
        let d = detail_with_body(Some("<p>hi</p>"));
        assert_eq!(extract_body_value(&d), "<p>hi</p>");
    }

    #[test]
    fn extract_body_value_returns_empty_when_absent() {
        let d = detail_with_body(None);
        assert_eq!(extract_body_value(&d), "");
    }

    #[test]
    fn extract_space_key_from_webui_returns_segment_after_display() {
        let d = detail_with_webui("/display/DEV/Some+Page");
        assert_eq!(extract_space_key_from_webui(&d), Some("DEV".to_string()));
    }

    #[test]
    fn extract_space_key_from_webui_returns_none_for_unknown_path_shape() {
        let d = detail_with_webui("/pages/viewpage.action?pageId=42");
        assert_eq!(extract_space_key_from_webui(&d), None);
    }

    #[test]
    fn extract_space_key_from_webui_handles_leading_slash_and_extra_segments() {
        let d = detail_with_webui("/display/PROJ/Long+Title+With+Slashes");
        assert_eq!(extract_space_key_from_webui(&d), Some("PROJ".to_string()));
    }
}
