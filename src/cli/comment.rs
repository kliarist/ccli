//! `ccli comment` subcommand handlers.
//!
//! Locked decisions implemented:
//! - D-51: $EDITOR receives plain text, NOT raw storage XML (intentional UX divergence from D-40).
//! - D-52: blank-line-separated paragraphs → `<p>...</p>` blocks before POST.
//! - D-53: editor template is empty (no placeholder).
//!
//! Security:
//! - page_id is validated digits-only before constructing /tmp paths (T-03-14).
//! - $EDITOR is launched via Command::new(editor).arg(path) — never via shell (T-03-EDITOR).
//! - PAT is in Client only — no #[instrument] here (CLI doesn't emit traces).

use anyhow::Context;

use crate::api::comment::add_comment;
use crate::api::{AppError, Client};
use crate::cli::{Cli, CommentArgs, CommentCommands};
use crate::config;

/// Top-level dispatcher for `ccli comment` subcommands.
pub async fn run(_cli: &Cli, args: &CommentArgs) -> anyhow::Result<()> {
    match &args.command {
        CommentCommands::Add { page_id } => handle_add(page_id).await,
    }
}

/// `ccli comment add <PAGE-ID>` — open $EDITOR, wrap, POST.
async fn handle_add(page_id: &str) -> anyhow::Result<()> {
    // T-03-14: digits-only validation prevents path traversal in /tmp/ccli-comment-{id}.txt
    let id = sanitize_id(page_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;

    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    // D-53: empty template — user starts typing immediately.
    let temp_path = std::env::temp_dir().join(format!("ccli-comment-{}.txt", id));
    std::fs::write(&temp_path, "")
        .with_context(|| format!("Failed to write template to {:?}", temp_path))?;
    launch_editor(&temp_path)?;
    let plain_text = std::fs::read_to_string(&temp_path)
        .with_context(|| format!("Failed to read comment from {:?}", temp_path))?;

    // D-52: wrap (and validate non-empty)
    let xml = match wrap_plain_text_to_storage_xml(&plain_text) {
        Ok(x) => x,
        Err(_) => {
            let _ = std::fs::remove_file(&temp_path);
            anyhow::bail!("comment add: editor returned empty content — comment not posted");
        }
    };

    add_comment(&client, &id, &xml)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to add comment")?;

    let _ = std::fs::remove_file(&temp_path);
    println!("Comment added to page {}.", id);
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Validate that `s` is a non-empty digit string. T-03-14.
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

/// Launch `$EDITOR` (or `$VISUAL`, or `vi`) — same pattern as src/cli/page.rs.
fn launch_editor(path: &std::path::Path) -> anyhow::Result<()> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to launch $EDITOR ({})", editor))?;
    if !status.success() {
        anyhow::bail!("$EDITOR exited non-zero (status {:?})", status.code());
    }
    Ok(())
}

/// D-52: wrap plain-text comment into storage XML.
///
/// Rules (per 04-UI-SPEC Plain-Text Wrapping Contract):
///  1. Split on blank lines (one or more consecutive `\n` after trimming whitespace-only).
///  2. Each non-empty paragraph → `<p>{escape(text)}</p>`; internal `\n` → space.
///  3. Escape order: `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`.
///  4. Empty / all-whitespace input → Err (caller suppresses the POST).
pub(crate) fn wrap_plain_text_to_storage_xml(text: &str) -> Result<String, &'static str> {
    // Split on lines, group runs of non-blank lines into paragraphs.
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join(" "));
                current.clear();
            }
        } else {
            current.push(line.trim());
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join(" "));
    }

    let paragraphs: Vec<String> = paragraphs
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect();

    if paragraphs.is_empty() {
        return Err("empty content");
    }

    let xml: String = paragraphs
        .iter()
        .map(|p| format!("<p>{}</p>", escape_xml(p)))
        .collect::<Vec<_>>()
        .join("");

    Ok(xml)
}

/// Escape XML special chars for storage-format text content.
/// `&` MUST be escaped first to avoid double-escaping the substitutions.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_plain_text_single_paragraph() {
        assert_eq!(
            wrap_plain_text_to_storage_xml("hello world").unwrap(),
            "<p>hello world</p>"
        );
    }

    #[test]
    fn wrap_plain_text_two_paragraphs() {
        assert_eq!(
            wrap_plain_text_to_storage_xml("p1\n\np2").unwrap(),
            "<p>p1</p><p>p2</p>"
        );
    }

    #[test]
    fn wrap_plain_text_three_paragraphs_with_extra_blank_lines() {
        assert_eq!(
            wrap_plain_text_to_storage_xml("p1\n\n\n\np2\n\np3").unwrap(),
            "<p>p1</p><p>p2</p><p>p3</p>"
        );
    }

    #[test]
    fn wrap_plain_text_escapes_xml_specials() {
        assert_eq!(
            wrap_plain_text_to_storage_xml("a < b & c > d").unwrap(),
            "<p>a &lt; b &amp; c &gt; d</p>"
        );
    }

    #[test]
    fn wrap_plain_text_empty_returns_error() {
        assert!(wrap_plain_text_to_storage_xml("").is_err());
        assert!(wrap_plain_text_to_storage_xml("   \n  \n").is_err());
    }

    #[test]
    fn wrap_plain_text_collapses_internal_newlines_within_paragraph() {
        assert_eq!(
            wrap_plain_text_to_storage_xml("line one\nline two").unwrap(),
            "<p>line one line two</p>"
        );
    }

    #[test]
    fn sanitize_id_rejects_path_traversal_inputs() {
        assert_eq!(sanitize_id("../etc"), None);
        assert_eq!(sanitize_id(""), None);
        assert_eq!(sanitize_id("12345"), Some("12345".to_string()));
    }
}
