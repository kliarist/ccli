//! `ccli comment` subcommand handlers.
//!
//! Security:
//! - page_id is validated digits-only before constructing /tmp paths (path traversal).
//! - $EDITOR is launched via Command::new(editor).arg(path) — never via shell.

use anyhow::Context;

use crate::api::comment::{add_comment, list_comments};
use crate::api::{AppError, Client};
use crate::cli::{sanitize_id, Cli, CommentArgs, CommentCommands};
use crate::config;
use crate::output::{strip_storage_xml, OutputFormatter};

/// Top-level dispatcher for `ccli comment` subcommands.
pub async fn run(cli: &Cli, args: &CommentArgs) -> anyhow::Result<()> {
    match &args.command {
        CommentCommands::List { page_id } => handle_list(cli, page_id).await,
        CommentCommands::Add { page_id } => handle_add(page_id).await,
    }
}

/// `ccli comment list <PAGE-ID>` — print all comments as a table.
async fn handle_list(cli: &Cli, page_id: &str) -> anyhow::Result<()> {
    let id = sanitize_id(page_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;

    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let comments = list_comments(&client, &id)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to fetch comments")?;

    let formatter = OutputFormatter::new(cli.output_config());
    let headers = &["ID", "Author", "Date", "Preview"];
    let rows = build_list_rows(&comments);
    formatter.print(headers, &rows);

    Ok(())
}

/// `ccli comment add <PAGE-ID>` — open $EDITOR, wrap, POST.
async fn handle_add(page_id: &str) -> anyhow::Result<()> {
    let id = sanitize_id(page_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;

    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let temp_path = std::env::temp_dir().join(format!("ccli-comment-{}.txt", id));
    std::fs::write(&temp_path, "")
        .with_context(|| format!("Failed to write template to {:?}", temp_path))?;
    launch_editor(&temp_path)?;
    let plain_text = std::fs::read_to_string(&temp_path)
        .with_context(|| format!("Failed to read comment from {:?}", temp_path))?;

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

/// Wrap plain-text comment into Confluence storage XML.
///
/// Rules:
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

pub(crate) fn build_list_rows(comments: &[crate::api::comment::Comment]) -> Vec<Vec<String>> {
    comments.iter().map(comment_to_row).collect()
}

fn comment_to_row(c: &crate::api::comment::Comment) -> Vec<String> {
    let author = c
        .version
        .as_ref()
        .and_then(|v| v.by.as_ref())
        .and_then(|a| a.display_name.clone())
        .unwrap_or_default();
    let date = c
        .version
        .as_ref()
        .and_then(|v| v.when.as_deref())
        .and_then(|w| w.get(..10))
        .unwrap_or_default()
        .to_string();
    let preview = c
        .body
        .as_ref()
        .and_then(|b| b.storage.as_ref())
        .and_then(|s| s.value.as_deref())
        .map(build_preview)
        .unwrap_or_default();
    vec![c.id.clone(), author, date, preview]
}

fn build_preview(xml: &str) -> String {
    let text = strip_storage_xml(xml);
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if first.chars().count() > 80 {
        format!("{}…", first.chars().take(79).collect::<String>())
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::comment::{
        Comment, CommentAuthor, CommentBody, CommentLinks, CommentVersion, StorageBody,
    };

    fn make_comment(
        id: &str,
        author: Option<&str>,
        when: Option<&str>,
        body: Option<&str>,
    ) -> Comment {
        Comment {
            id: id.to_string(),
            title: String::new(),
            version: Some(CommentVersion {
                number: 1,
                when: when.map(ToString::to_string),
                by: Some(CommentAuthor {
                    display_name: author.map(ToString::to_string),
                }),
            }),
            body: Some(CommentBody {
                storage: Some(StorageBody {
                    value: body.map(ToString::to_string),
                    representation: Some("storage".to_string()),
                }),
            }),
            links: CommentLinks::default(),
        }
    }

    #[test]
    fn escape_xml_escapes_all_three_specials() {
        assert_eq!(escape_xml("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn escape_xml_escapes_ampersand_first_to_avoid_double_encoding() {
        // If & were escaped after < the result would be "&amp;lt;" — must escape & first.
        assert_eq!(escape_xml("a & b < c"), "a &amp; b &lt; c");
    }

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
    fn wrap_plain_text_collapses_internal_newlines_within_paragraph() {
        assert_eq!(
            wrap_plain_text_to_storage_xml("line one\nline two").unwrap(),
            "<p>line one line two</p>"
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
    fn wrap_plain_text_whitespace_only_paragraph_is_excluded() {
        assert_eq!(
            wrap_plain_text_to_storage_xml("real paragraph\n\n   \n\nanother").unwrap(),
            "<p>real paragraph</p><p>another</p>"
        );
    }

    #[test]
    fn wrap_plain_text_empty_returns_error() {
        assert!(wrap_plain_text_to_storage_xml("").is_err());
        assert!(wrap_plain_text_to_storage_xml("   \n  \n").is_err());
    }

    #[test]
    fn build_list_rows_formats_author_date_and_preview() {
        let comments = vec![make_comment(
            "42",
            Some("Alice"),
            Some("2026-05-01T12:34:56Z"),
            Some("<p>Hello</p><p>World</p>"),
        )];

        let rows = build_list_rows(&comments);

        assert_eq!(
            rows,
            vec![vec![
                "42".to_string(),
                "Alice".to_string(),
                "2026-05-01".to_string(),
                "Hello".to_string(),
            ]]
        );
    }

    #[test]
    fn build_list_rows_uses_defaults_for_missing_comment_fields() {
        let comments = vec![Comment {
            id: "7".to_string(),
            title: String::new(),
            version: None,
            body: None,
            links: CommentLinks::default(),
        }];

        let rows = build_list_rows(&comments);

        assert_eq!(
            rows,
            vec![vec![
                "7".to_string(),
                String::new(),
                String::new(),
                String::new(),
            ]]
        );
    }

    #[test]
    fn build_list_rows_truncates_long_preview_to_80_chars() {
        let long_text = "x".repeat(100);
        let comments = vec![make_comment(
            "9",
            Some("Bob"),
            Some("2026-05-01T00:00:00Z"),
            Some(&format!("<p>{}</p>", long_text)),
        )];

        let rows = build_list_rows(&comments);
        let preview = &rows[0][3];

        assert_eq!(preview.chars().count(), 80);
        assert!(preview.ends_with('…'));
    }
}
