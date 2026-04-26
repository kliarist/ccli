//! `ccli init` interactive setup flow.
//!
//! Locked decisions implemented:
//! - D-01 dialoguer for prompts (ColorfulTheme + Input + Password)
//! - D-02 connection test BEFORE save; do not persist on failure
//! - D-03 pre-fill URL on re-run; show masked PAT hint; Enter to keep
//! - Pitfall 4 (RESEARCH.md): empty Password::interact() result on re-run = keep existing
//!
//! Security:
//! - Token value never appears in stdout/stderr or tracing output. The
//!   masked hint is the ONLY surfaced derivative of the existing token.
//! - URL is validated to start with `http://` or `https://` before being
//!   accepted by the prompt (extends T-04-04 mitigation from Plan 04).

use anyhow::Context;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password};

use crate::api;
use crate::cli::Cli;
use crate::config::{self, Config};

/// Top-level handler for `ccli init`.
pub async fn run(_cli: &Cli) -> anyhow::Result<()> {
    let existing = config::load().context("Failed to load existing config")?;
    let theme = ColorfulTheme::default();

    // --- URL prompt: pre-fill on re-run (D-03) + scheme validation ---
    let url: String = Input::with_theme(&theme)
        .with_prompt("Confluence URL")
        .with_initial_text(
            existing.as_ref().map(|c| c.url.as_str()).unwrap_or(""),
        )
        .validate_with(|s: &String| -> Result<(), &'static str> {
            validate_url(s)
        })
        .interact_text()
        .context("URL prompt failed")?;

    // --- PAT prompt: masked input + hint on re-run (D-03 + Pitfall 4) ---
    let pat_prompt = match existing.as_ref() {
        Some(c) => format!(
            "Personal Access Token [current: {}]",
            mask_pat_hint(&c.token)
        ),
        None => "Personal Access Token".to_string(),
    };

    let raw_pat: String = Password::with_theme(&theme)
        .with_prompt(&pat_prompt)
        .allow_empty_password(true)
        .interact()
        .context("PAT prompt failed")?;

    // Pitfall 4: empty input on re-run = keep existing token; on first run = error
    let token = if raw_pat.is_empty() {
        existing
            .as_ref()
            .map(|c| c.token.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("A Personal Access Token is required on first run.")
            })?
    } else {
        raw_pat
    };

    // --- Connection test BEFORE save (D-02) ---
    let display_name = api::test_connection(&url, &token)
        .await
        .context("Connection test failed")?;

    // --- Save only on success (D-02) ---
    let cfg = Config {
        url: url.clone(),
        token,
    };
    config::save(&cfg).context("Failed to save config")?;

    println!(
        "Connected as {} — configuration saved.",
        display_name
    );
    Ok(())
}

/// Validate the user-entered Confluence URL.
///
/// Phase 1 enforces the scheme prefix only. Future phases may add fully-qualified
/// URL parsing via the `url` crate, but for now this matches the validation
/// captured in RESEARCH.md Pattern 2 and CONTEXT.md security guidance.
pub(crate) fn validate_url(s: &str) -> Result<(), &'static str> {
    if s.starts_with("http://") || s.starts_with("https://") {
        Ok(())
    } else {
        Err("URL must start with http:// or https://")
    }
}

/// Produce a non-revealing visual hint of an existing PAT.
///
/// Format: `<first2>*****<last3>` for tokens of length >= 6.
/// For tokens of length 5 or fewer, return only stars (one per character) —
/// no character of the token is revealed.
///
/// Examples:
///   "ATATT3xFfGF0abc123" -> "AT*****123"
///   "AB"                 -> "**"
///   ""                   -> ""
///   "ABCDEF"             -> "AB*****DEF"
pub(crate) fn mask_pat_hint(token: &str) -> String {
    let len = token.chars().count();
    if len <= 5 {
        return "*".repeat(len);
    }
    // For len >= 6 the slicing is safe: take leading 2 bytes + trailing 3 bytes.
    // PATs are ASCII, so byte indexing aligns with char indexing.
    let prefix = &token[..2];
    let suffix = &token[token.len() - 3..];
    format!("{}*****{}", prefix, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- mask_pat_hint tests ---

    #[test]
    fn mask_pat_hint_long_token_first2_stars_last3() {
        assert_eq!(mask_pat_hint("ATATT3xFfGF0abc123"), "AT*****123");
    }

    #[test]
    fn mask_pat_hint_short_token_full_mask() {
        assert_eq!(mask_pat_hint("short"), "*****");
    }

    #[test]
    fn mask_pat_hint_empty_input_empty_output() {
        assert_eq!(mask_pat_hint(""), "");
    }

    #[test]
    fn mask_pat_hint_two_chars_two_stars() {
        assert_eq!(mask_pat_hint("AB"), "**");
    }

    #[test]
    fn mask_pat_hint_six_chars_first2_stars_last3() {
        assert_eq!(mask_pat_hint("ABCDEF"), "AB*****DEF");
    }

    #[test]
    fn mask_pat_hint_never_contains_full_token() {
        // Property: for any non-empty input, the hint MUST NOT equal the input.
        let inputs = ["a", "ab", "abc", "abcd", "abcde", "abcdef", "abcdefghij"];
        for i in inputs {
            assert_ne!(mask_pat_hint(i), i, "hint must not be identical to token: {}", i);
        }
    }

    // --- validate_url tests ---

    #[test]
    fn validate_url_https_ok() {
        assert!(validate_url("https://example.com").is_ok());
    }

    #[test]
    fn validate_url_http_ok() {
        assert!(validate_url("http://example.com").is_ok());
    }

    #[test]
    fn validate_url_ftp_rejected() {
        let err = validate_url("ftp://example.com").unwrap_err();
        assert!(err.contains("http://") || err.contains("https://"));
    }

    #[test]
    fn validate_url_no_scheme_rejected() {
        assert!(validate_url("example.com").is_err());
    }

    #[test]
    fn validate_url_empty_rejected() {
        assert!(validate_url("").is_err());
    }
}
