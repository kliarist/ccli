//! Pandoc-backed editor session for Confluence storage XML.
//!
//! When pandoc is available and `raw` is false:
//!   storage XML → pandoc (html→gfm) → .md temp file → $EDITOR
//!   → pandoc (gfm→html) → storage XML → Confluence API
//!
//! Fallback (pandoc absent or `raw = true`):
//!   storage XML → .xml temp file → $EDITOR → storage XML → Confluence API

use std::io::Write as _;

use anyhow::Context;

/// True if `pandoc` is found and executable on `$PATH`.
pub fn pandoc_available() -> bool {
    std::process::Command::new("pandoc")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Confluence storage XML → GitHub-Flavored Markdown via pandoc.
fn xml_to_markdown(xml: &str) -> anyhow::Result<String> {
    run_pandoc(&["-f", "html", "-t", "gfm", "--wrap=none"], xml)
        .context("pandoc html→gfm conversion failed")
}

/// GitHub-Flavored Markdown → HTML (Confluence storage format) via pandoc.
fn markdown_to_xml(md: &str) -> anyhow::Result<String> {
    run_pandoc(&["-f", "gfm", "-t", "html", "--wrap=none"], md)
        .context("pandoc gfm→html conversion failed")
}

/// Spawn pandoc with `args`, pipe `input` to stdin, return stdout as a String.
fn run_pandoc(args: &[&str], input: &str) -> anyhow::Result<String> {
    let mut child = std::process::Command::new("pandoc")
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn pandoc")?;

    child
        .stdin
        .as_mut()
        .expect("stdin piped")
        .write_all(input.as_bytes())
        .context("Failed to write to pandoc stdin")?;

    let out = child
        .wait_with_output()
        .context("Failed to wait for pandoc")?;

    if !out.status.success() {
        anyhow::bail!("pandoc exited with status {:?}", out.status.code());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Launch `$EDITOR` (or `$VISUAL`, or `vi`) on `path`, blocking until exit.
/// Never invokes a shell — uses `Command::new(editor).arg(path)`.
pub(crate) fn launch_editor(path: &std::path::Path) -> anyhow::Result<()> {
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

/// Run a full edit session and return the new Confluence storage XML.
///
/// If `raw` is false and pandoc is available the body is converted to Markdown
/// before opening the editor and back to storage XML afterwards.  When pandoc
/// is absent or `raw` is true the raw storage XML is opened directly.
///
/// The `page_id` is used only to name the temp file (no shell interpolation).
pub fn run_edit_session(body_xml: &str, page_id: &str, raw: bool) -> anyhow::Result<String> {
    if !raw && pandoc_available() {
        let markdown = xml_to_markdown(body_xml)?;
        let temp_path = std::env::temp_dir().join(format!("ccli-edit-{}.md", page_id));
        std::fs::write(&temp_path, &markdown)
            .with_context(|| format!("Failed to write temp file {}", temp_path.display()))?;
        launch_editor(&temp_path)?;
        let edited_md = std::fs::read_to_string(&temp_path)
            .with_context(|| format!("Failed to read edited file {}", temp_path.display()))?;
        markdown_to_xml(&edited_md)
    } else {
        let temp_path = std::env::temp_dir().join(format!("ccli-edit-{}.xml", page_id));
        std::fs::write(&temp_path, body_xml)
            .with_context(|| format!("Failed to write temp file {}", temp_path.display()))?;
        launch_editor(&temp_path)?;
        std::fs::read_to_string(&temp_path)
            .with_context(|| format!("Failed to read edited file {}", temp_path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_edit_session_raw_returns_error_when_editor_missing() {
        let path_guard = std::env::temp_dir().join("ccli-test-editor-fail.xml");
        std::env::set_var("EDITOR", "this-binary-does-not-exist-xyzzy-12345");
        let result = run_edit_session("<p>test</p>", "99999", true);
        std::env::remove_var("EDITOR");
        assert!(result.is_err(), "expected editor launch failure to be Err");
        let _ = std::fs::remove_file(&path_guard);
    }

    #[test]
    fn pandoc_available_does_not_panic() {
        let _ = pandoc_available();
    }

    #[test]
    fn xml_to_markdown_converts_paragraph_when_pandoc_present() {
        if !pandoc_available() {
            return;
        }
        let md = xml_to_markdown("<p>Hello world</p>").expect("conversion ok");
        assert!(
            md.contains("Hello world"),
            "markdown should contain original text; got: {}",
            md
        );
    }

    #[test]
    fn markdown_to_xml_converts_paragraph_when_pandoc_present() {
        if !pandoc_available() {
            return;
        }
        let html = markdown_to_xml("Hello world\n").expect("conversion ok");
        assert!(
            html.contains("Hello world"),
            "html should contain original text; got: {}",
            html
        );
    }

    #[test]
    fn run_edit_session_raw_returns_original_when_editor_is_true() {
        // /bin/true exits 0 without modifying the file — content is preserved as-is.
        if !std::path::Path::new("/bin/true").exists() {
            return; // not a Unix system
        }
        std::env::set_var("EDITOR", "/bin/true");
        let result = run_edit_session("<p>unchanged</p>", "test-raw-noop", true);
        std::env::remove_var("EDITOR");
        assert_eq!(
            result.expect("ok"),
            "<p>unchanged</p>",
            "raw session with no-op editor should return original content"
        );
    }
}
