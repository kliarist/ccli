//! `ccli attachment` subcommand handlers.
//!
//! Locked decisions implemented:
//! - ATTCH-01 / D-56: `attachment list` — table (Filename, Size, Media-Type, Date)
//! - ATTCH-02 / D-54: `attachment get` — download to ./FILENAME, --output overrides
//! - ATTCH-03 / D-55: `attachment add` — multipart upload with auto-detected MIME
//!
//! Security:
//! - page_id validated digits-only (T-03-14).
//! - file_path is read with std::fs::read at the API layer; we surface I/O errors
//!   to stderr but do not interpret the path further.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::api::attachment::{add_attachment, get_attachment, list_attachments, Attachment};
use crate::api::{AppError, Client};
use crate::cli::{sanitize_id, AttachmentArgs, AttachmentCommands, Cli};
use crate::config;
use crate::output::OutputFormatter;

/// Top-level dispatcher for `ccli attachment` subcommands.
pub async fn run(cli: &Cli, args: &AttachmentArgs) -> anyhow::Result<()> {
    match &args.command {
        AttachmentCommands::List { page_id } => handle_list(cli, page_id).await,
        AttachmentCommands::Get {
            page_id,
            filename,
            output,
        } => handle_get(page_id, filename, output.as_deref()).await,
        AttachmentCommands::Add { page_id, file_path } => handle_add(page_id, file_path).await,
    }
}

// ─── List (ATTCH-01, D-56) ────────────────────────────────────────────────────

async fn handle_list(cli: &Cli, page_id: &str) -> anyhow::Result<()> {
    let id = sanitize_id(page_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let attachments = list_attachments(&client, &id)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to list attachments")?;

    let formatter = OutputFormatter::new(cli.output_config());
    let headers = &["Filename", "Size", "Media-Type", "Date"];
    let rows = build_list_rows(&attachments);
    formatter.print(headers, &rows);
    Ok(())
}

/// Pure helper — extracted for unit testability (no I/O).
pub(crate) fn build_list_rows(attachments: &[Attachment]) -> Vec<Vec<String>> {
    attachments
        .iter()
        .map(|a| {
            let size_bytes = a.extensions.as_ref().and_then(|e| e.file_size).unwrap_or(0);
            let media = a
                .extensions
                .as_ref()
                .and_then(|e| e.media_type.clone())
                .unwrap_or_default();
            let when = a
                .version
                .as_ref()
                .and_then(|v| v.when.as_deref())
                .map(|w| w.get(..10).unwrap_or(w).to_string())
                .unwrap_or_default();
            vec![a.title.clone(), human_size(size_bytes), media, when]
        })
        .collect()
}

// ─── Get (ATTCH-02, D-54) ─────────────────────────────────────────────────────

async fn handle_get(page_id: &str, filename: &str, output: Option<&str>) -> anyhow::Result<()> {
    let id = sanitize_id(page_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let bytes = get_attachment(&client, &id, filename)
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| format!("attachment get: '{}' on page {}", filename, id))?;

    let dest: PathBuf = output
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(".").join(filename));

    std::fs::write(&dest, &bytes)
        .with_context(|| format!("attachment get: could not write '{}'", dest.display()))?;

    println!("Downloaded: {} \u{2192} {}", filename, dest.display());
    Ok(())
}

// ─── Add (ATTCH-03, D-55) ─────────────────────────────────────────────────────

async fn handle_add(page_id: &str, file_path: &str) -> anyhow::Result<()> {
    let id = sanitize_id(page_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;
    let path = Path::new(file_path);
    if !path.exists() {
        anyhow::bail!("attachment add: '{}' not found", file_path);
    }
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("attachment add: cannot stat '{}'", file_path))?;
    let size = metadata.len();
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload.bin")
        .to_string();

    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    add_attachment(&client, &id, path)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to upload attachment")?;

    println!(
        "Uploaded: {} ({}) \u{2192} page {}",
        filename,
        human_size(size),
        id
    );
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Human-readable file size. Boundary 1024.
/// <1024 → "N B"; <1MiB → "N KB"; else "N MB". Round to nearest.
pub(crate) fn human_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b < KIB {
        format!("{} B", bytes)
    } else if b < MIB {
        format!("{} KB", (b / KIB).round() as u64)
    } else {
        format!("{} MB", (b / MIB).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::attachment::{
        Attachment, AttachmentExtensions, AttachmentLinks, AttachmentVersion,
    };

    fn make_attachment(id: &str, title: &str, size: u64, media: &str, when: &str) -> Attachment {
        Attachment {
            id: id.to_string(),
            title: title.to_string(),
            extensions: Some(AttachmentExtensions {
                media_type: Some(media.to_string()),
                file_size: Some(size),
            }),
            version: Some(AttachmentVersion {
                when: Some(when.to_string()),
            }),
            links: AttachmentLinks::default(),
        }
    }

    #[test]
    fn human_size_formats_bytes_under_1024() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
    }

    #[test]
    fn human_size_formats_kibibytes() {
        assert_eq!(human_size(1024), "1 KB");
        assert_eq!(human_size(145408), "142 KB");
    }

    #[test]
    fn human_size_formats_mebibytes() {
        assert_eq!(human_size(1048576), "1 MB");
        assert_eq!(human_size(10_485_760), "10 MB");
    }

    #[test]
    fn human_size_rounds_to_nearest() {
        assert_eq!(human_size(1500), "1 KB"); // 1.46 → 1
        assert_eq!(human_size(1600), "2 KB"); // 1.56 → 2
    }

    #[test]
    fn attachment_list_rows_have_4_columns() {
        let a1 = make_attachment(
            "1",
            "design.png",
            145408,
            "image/png",
            "2026-05-01T10:00:00Z",
        );
        let a2 = make_attachment(
            "2",
            "spec.pdf",
            1048576,
            "application/pdf",
            "2026-04-30T08:00:00Z",
        );
        let rows = build_list_rows(&[a1, a2]);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            vec![
                "design.png".to_string(),
                "142 KB".to_string(),
                "image/png".to_string(),
                "2026-05-01".to_string(),
            ]
        );
        assert_eq!(
            rows[1],
            vec![
                "spec.pdf".to_string(),
                "1 MB".to_string(),
                "application/pdf".to_string(),
                "2026-04-30".to_string(),
            ]
        );
    }
}
