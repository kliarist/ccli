pub mod path;

use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::api::error::AppError;
use path::config_path;

/// Single-profile config (D-09): one URL + one token, flat schema.
/// `email` is only present for Confluence Cloud (*.atlassian.net) instances,
/// where authentication uses Basic Auth (email + API token) instead of Bearer PAT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Load config from the canonical path with `CCLI_URL` / `CCLI_TOKEN` env var overlay (INIT-03).
///
/// Precedence (W-06 fix — natural reading of ROADMAP success criterion 2 "User can override
/// config by setting CCLI_URL and CCLI_TOKEN"):
/// - file exists, env unset       -> Some(file_config)
/// - file exists, env set         -> Some(file_config) with env values overlaid (env wins)
/// - no file, both env vars set   -> Some(synthesized config from env)        [env-only mode]
/// - no file, partial/empty env   -> None
/// - no file, no env              -> None
///
/// `load_or_error` converts None into AppError::Config.
pub fn load() -> anyhow::Result<Option<Config>> {
    let path = config_path()?;
    load_from(&path)
}

/// Testable variant of `load()` accepting an explicit path.
///
/// Implements the W-06 env-only synthesis branch: when the file does not exist but
/// BOTH `CCLI_URL` and `CCLI_TOKEN` are set to non-empty values, return a Config
/// built from the env vars. This makes "User can override config by setting CCLI_URL
/// and CCLI_TOKEN" (ROADMAP success criterion 2) work without a pre-existing file.
pub(crate) fn load_from(path: &Path) -> anyhow::Result<Option<Config>> {
    let env_url = std::env::var("CCLI_URL").ok().filter(|s| !s.is_empty());
    let env_token = std::env::var("CCLI_TOKEN").ok().filter(|s| !s.is_empty());
    let env_email = std::env::var("CCLI_EMAIL").ok().filter(|s| !s.is_empty());

    if !path.exists() {
        // W-06: env-only synthesis. Both vars must be present and non-empty;
        // a partial environment is not sufficient and falls through to None.
        return Ok(match (env_url, env_token) {
            (Some(url), Some(token)) => Some(Config {
                url,
                token,
                email: env_email,
            }),
            _ => None,
        });
    }

    let content = fs::read_to_string(path)?;
    let mut config: Config = toml::from_str(&content)?;

    // Env var overrides (INIT-03) — only non-empty values override.
    if let Some(url) = env_url {
        config.url = url;
    }
    if let Some(token) = env_token {
        config.token = token;
    }
    if let Some(email) = env_email {
        config.email = Some(email);
    }
    Ok(Some(config))
}

/// Save config atomically and restrict permissions to owner-only on Unix.
///
/// Atomic write (Pitfall 6): write to `<path>.tmp` then `rename` to final path.
/// Permissions (Security ASVS V2): `chmod 0o600` on Unix after rename.
pub fn save(config: &Config) -> anyhow::Result<()> {
    let path = config_path()?;
    save_to(&path, config)
}

/// Testable variant of `save()` accepting an explicit path.
pub(crate) fn save_to(path: &Path, config: &Config) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;

    // Build a sibling temp path: <stem>.toml.tmp
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, &content)?;
    fs::rename(&tmp, path)?;

    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Load config or return `AppError::Config` with a clear remediation message.
///
/// Used by every command that requires Confluence credentials. Plan 05 init
/// uses `load()` directly because absence is an expected branch there.
pub fn load_or_error() -> Result<Config, AppError> {
    let path = config_path().map_err(|e| AppError::Config(e.to_string()))?;
    load_or_error_from(&path)
}

/// Testable variant of `load_or_error()` accepting an explicit path (W-05 fix).
///
/// The public `load_or_error()` is a thin wrapper around this helper, so tests
/// can exercise the AppError mapping with a TempDir-based path and never need
/// to mutate `$HOME`.
pub(crate) fn load_or_error_from(path: &Path) -> Result<Config, AppError> {
    load_from(path)
        .map_err(|e| AppError::Config(e.to_string()))?
        .ok_or_else(|| {
            AppError::Config("No configuration found. Run 'ccli init' to configure.".to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Serialize tests that mutate CCLI_URL / CCLI_TOKEN — Rust runs unit tests in parallel.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn fixture_config() -> Config {
        Config {
            url: "https://confluence.example.com".into(),
            token: "AT-test-token-xyz".into(),
            email: None,
        }
    }

    /// Clear CCLI_URL and CCLI_TOKEN for the duration of the held guard, restoring
    /// any pre-existing values on drop. Required because `load_from` peeks at the
    /// real environment.
    struct EnvIsolation {
        prior_url: Option<String>,
        prior_token: Option<String>,
    }

    impl EnvIsolation {
        fn new() -> Self {
            let prior_url = std::env::var("CCLI_URL").ok();
            let prior_token = std::env::var("CCLI_TOKEN").ok();
            std::env::remove_var("CCLI_URL");
            std::env::remove_var("CCLI_TOKEN");
            Self {
                prior_url,
                prior_token,
            }
        }
    }

    impl Drop for EnvIsolation {
        fn drop(&mut self) {
            match self.prior_url.take() {
                Some(v) => std::env::set_var("CCLI_URL", v),
                None => std::env::remove_var("CCLI_URL"),
            }
            match self.prior_token.take() {
                Some(v) => std::env::set_var("CCLI_TOKEN", v),
                None => std::env::remove_var("CCLI_TOKEN"),
            }
        }
    }

    #[test]
    fn toml_roundtrip() {
        let cfg = fixture_config();
        let s = toml::to_string_pretty(&cfg).expect("serialize");
        let parsed: Config = toml::from_str(&s).expect("deserialize");
        assert_eq!(parsed.url, cfg.url);
        assert_eq!(parsed.token, cfg.token);
    }

    #[test]
    fn load_from_returns_none_when_missing_and_no_env() {
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.toml");
        let result = load_from(&path).expect("load_from should not error on missing");
        assert!(result.is_none());
    }

    #[test]
    fn save_to_creates_file_atomically() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ccli").join("config.toml");
        save_to(&path, &fixture_config()).expect("save");
        assert!(path.exists(), "config.toml should exist after save");
        let tmp = path.with_extension("toml.tmp");
        assert!(
            !tmp.exists(),
            "tmp file should NOT exist after save (rename consumed it)"
        );
    }

    #[test]
    fn save_then_load_roundtrip() {
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = fixture_config();
        save_to(&path, &cfg).expect("save");
        let loaded = load_from(&path).expect("load").expect("Some");
        assert_eq!(loaded.url, cfg.url);
        assert_eq!(loaded.token, cfg.token);
    }

    #[test]
    fn ccli_url_env_var_overrides_file_value() {
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        save_to(&path, &fixture_config()).expect("save");

        std::env::set_var("CCLI_URL", "https://override.example.com");
        let loaded = load_from(&path).expect("load").expect("Some");
        assert_eq!(loaded.url, "https://override.example.com");
        // token still from file
        assert_eq!(loaded.token, fixture_config().token);
    }

    #[test]
    fn ccli_token_env_var_overrides_file_value() {
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        save_to(&path, &fixture_config()).expect("save");

        std::env::set_var("CCLI_TOKEN", "AT-override-zzz");
        let loaded = load_from(&path).expect("load").expect("Some");
        assert_eq!(loaded.token, "AT-override-zzz");
        assert_eq!(loaded.url, fixture_config().url);
    }

    #[cfg(unix)]
    #[test]
    fn save_to_sets_0600_permissions_on_unix() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        save_to(&path, &fixture_config()).expect("save");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0o600, got {:o}", mode);
    }

    #[test]
    fn load_or_error_from_returns_config_variant_when_missing() {
        // W-05 fix: test load_or_error_from directly with a TempDir path,
        // no HOME mutation required.
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let result = load_or_error_from(&path);
        match result {
            Err(AppError::Config(msg)) => {
                assert!(
                    msg.contains("ccli init"),
                    "msg should mention ccli init: {}",
                    msg
                );
            }
            other => panic!("expected AppError::Config, got {:?}", other),
        }
    }

    // --- W-06 env-only synthesis tests ---

    #[test]
    fn load_from_env_only_synthesizes_config_when_both_vars_set_and_file_missing() {
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.toml");

        std::env::set_var("CCLI_URL", "https://env-only.example.com");
        std::env::set_var("CCLI_TOKEN", "AT-env-only-token");

        let result = load_from(&path)
            .expect("load")
            .expect("Some(env-only synthesized)");
        assert_eq!(result.url, "https://env-only.example.com");
        assert_eq!(result.token, "AT-env-only-token");
    }

    #[test]
    fn load_from_partial_env_only_returns_none_when_file_missing() {
        // Only CCLI_URL set — env-only mode requires BOTH.
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.toml");

        std::env::set_var("CCLI_URL", "https://partial.example.com");

        let result = load_from(&path).expect("load");
        assert!(
            result.is_none(),
            "partial env-only should not synthesize a config"
        );
    }

    #[test]
    fn load_from_empty_env_only_returns_none_when_file_missing() {
        // Empty values for either var should fall through to None.
        let _g = ENV_LOCK.lock().unwrap();
        let _iso = EnvIsolation::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.toml");

        std::env::set_var("CCLI_URL", "");
        std::env::set_var("CCLI_TOKEN", "AT-non-empty");

        let result = load_from(&path).expect("load");
        assert!(
            result.is_none(),
            "empty CCLI_URL should not satisfy env-only mode"
        );
    }
}
