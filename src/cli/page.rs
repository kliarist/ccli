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
use crate::cli::{Cli, PageArgs, PageCommands};
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
        PageCommands::Create { space, title, parent } => {
            handle_create_typed(
                space.as_deref(),
                title.as_deref(),
                parent.as_deref(),
                ContentType::Page,
            ).await
        }
        PageCommands::Edit { page_id } => {
            handle_edit_typed(page_id, ContentType::Page).await
        }
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
    let id = sanitize_id(content_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", content_id)))?;
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
    launch_editor(&temp_path)?;
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
/// On HTTP 409: stderr prints conflict message and the temp file path; temp file
/// is NOT deleted so the user can re-open and merge manually.
pub async fn handle_edit_typed(content_id: &str, content_type: ContentType) -> anyhow::Result<()> {
    let id = sanitize_id(content_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", content_id)))?;
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let detail = get_page_detail(&client, &id)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to fetch page detail before edit")?;
    let current_version = detail.version.as_ref().map(|v| v.number).unwrap_or(1);
    let title = detail.title.clone();
    // space.key is not part of PageDetail directly in our struct, but it IS required by update_page.
    // We extract it from the API at call time. For now, look it up from the ancestors chain or
    // require the user to supply it — but Confluence's PUT works as long as space matches the
    // existing page's space, so we infer from the GET response if available. The `_links.webui`
    // path always includes the space key (e.g. "/display/DEV/Page+Title") — extract from it.
    let space_key = extract_space_key_from_webui(&detail).ok_or_else(|| {
        AppError::Api(
            "Could not determine space key from page _links.webui — page may be missing _links data.".to_string(),
        )
    })?;

    let body = extract_body_value(&detail);

    let temp_path = std::path::PathBuf::from(format!("/tmp/ccli-edit-{}.xml", id));
    std::fs::write(&temp_path, &body)
        .with_context(|| format!("Failed to write current content to {:?}", temp_path))?;

    launch_editor(&temp_path)?;

    let new_xml = std::fs::read_to_string(&temp_path)
        .with_context(|| format!("Failed to read edited content from {:?}", temp_path))?;

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
            // Best-effort cleanup on success
            let _ = std::fs::remove_file(&temp_path);
            println!("Saved.");
            Ok(())
        }
        Err(AppError::Api(msg)) if msg.starts_with("Conflict:") => {
            // D-39: preserve temp file, print path on stderr
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

/// Validate that `s` is a non-empty digit string. Returns Some(s) when valid,
/// None when not. Prevents path traversal in /tmp/ccli-edit-{id}.xml.
fn sanitize_id(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        Some(s.to_string())
    } else {
        None
    }
}

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

/// Launch `$EDITOR` (or `$VISUAL`, or `vi`) on the given path, blocking until exit.
fn launch_editor(path: &std::path::Path) -> anyhow::Result<()> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    // Security: never via shell — arg() prevents injection (T-03-EDITOR).
    let status = std::process::Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to launch $EDITOR ({})", editor))?;
    if !status.success() {
        anyhow::bail!("$EDITOR exited non-zero (status {:?})", status.code());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::page::{PageBody, PageDetail, PageLinks, StorageBody};

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
