//! `ccli init` interactive setup flow.
//!
//! Supports both Confluence Cloud (*.atlassian.net) and Data Center / Server:
//! - Cloud: prompts for email + API token; uses Basic Auth
//! - DC/Server: prompts for PAT; uses Bearer Auth
//!
//! Security:
//! - Token value never appears in stdout/stderr or tracing output.
//! - URL is validated (scheme check) and normalized before use.

use anyhow::Context;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password};
use is_terminal::IsTerminal;

use crate::api;
use crate::cli::Cli;
use crate::config::{self, Config};

/// Top-level handler for `ccli init`.
pub async fn run(_cli: &Cli) -> anyhow::Result<()> {
    let existing = config::load().context("Failed to load existing config")?;
    let theme = ColorfulTheme::default();

    // --- URL prompt: pre-fill on re-run + scheme validation ---
    let raw_url: String = Input::with_theme(&theme)
        .with_prompt("Confluence URL")
        .with_initial_text(existing.as_ref().map(|c| c.url.as_str()).unwrap_or(""))
        .validate_with(|s: &String| -> Result<(), &'static str> { validate_url(s) })
        .interact_text()
        .context("URL prompt failed")?;

    let url = api::normalize_url(&raw_url);
    let is_cloud = api::is_cloud_url(&url);

    // --- Email prompt (Cloud only) ---
    let email: Option<String> = if is_cloud {
        let existing_email = existing
            .as_ref()
            .and_then(|c| c.email.as_deref())
            .unwrap_or("");
        let raw: String = Input::with_theme(&theme)
            .with_prompt("Atlassian account email")
            .with_initial_text(existing_email)
            .interact_text()
            .context("Email prompt failed")?;
        Some(raw)
    } else {
        None
    };

    // --- Token prompt: label varies by instance type ---
    let token_label = if is_cloud {
        match existing.as_ref().filter(|_| email.is_some()) {
            Some(c) if c.email == email => {
                format!("Atlassian API Token [current: {}]", mask_pat_hint(&c.token))
            }
            _ => "Atlassian API Token".to_string(),
        }
    } else {
        match existing.as_ref() {
            Some(c) => format!(
                "Personal Access Token [current: {}]",
                mask_pat_hint(&c.token)
            ),
            None => "Personal Access Token".to_string(),
        }
    };

    let raw_token: String = Password::with_theme(&theme)
        .with_prompt(&token_label)
        .allow_empty_password(true)
        .interact()
        .context("Token prompt failed")?;

    // Empty input on re-run = keep existing token; on first run = error
    let token = if raw_token.is_empty() {
        existing.as_ref().map(|c| c.token.clone()).ok_or_else(|| {
            if is_cloud {
                anyhow::anyhow!("An Atlassian API Token is required on first run.")
            } else {
                anyhow::anyhow!("A Personal Access Token is required on first run.")
            }
        })?
    } else {
        raw_token
    };

    // --- Connection test BEFORE save ---
    let display_name = api::test_connection(&url, &token, email.as_deref())
        .await
        .context("Connection test failed")?;

    // --- Save only on success ---
    let cfg = Config {
        url: url.clone(),
        token,
        email,
    };
    config::save(&cfg).context("Failed to save config")?;

    if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        println!(
            "\x1b[1;32mConnected as {}\x1b[0m — configuration saved.",
            display_name
        );
    } else {
        println!("Connected as {} — configuration saved.", display_name);
    }
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
    let chars: Vec<char> = token.chars().collect();
    let len = chars.len();
    if len <= 5 {
        return "*".repeat(len);
    }
    let prefix: String = chars[..2].iter().collect();
    let suffix: String = chars[len - 3..].iter().collect();
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
            assert_ne!(
                mask_pat_hint(i),
                i,
                "hint must not be identical to token: {}",
                i
            );
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
