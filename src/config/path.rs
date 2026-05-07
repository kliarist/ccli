use std::path::PathBuf;

/// Returns the canonical config file path: `~/.config/ccli/config.toml`.
///
/// IMPORTANT: This function uses `home::home_dir()`, NOT `dirs::config_dir()`.
/// In `dirs` 6.0+, `config_dir()` returns `~/Library/Application Support` on macOS,
/// which contradicts the locked path requirement (CONTEXT.md INIT-02 / D-09).
/// All Phase 1+ code must call this function to obtain the config path —
/// never construct it inline.
pub fn config_path() -> anyhow::Result<PathBuf> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".config").join("ccli").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_with_config_ccli_config_toml() {
        let p = config_path().expect("home_dir should be resolvable in test env");
        let s = p.to_string_lossy();
        assert!(
            s.ends_with(".config/ccli/config.toml"),
            "expected path ending in .config/ccli/config.toml, got {}",
            s
        );
    }

    #[test]
    fn is_absolute() {
        let p = config_path().expect("home_dir should be resolvable in test env");
        assert!(p.is_absolute(), "expected absolute path, got {:?}", p);
    }

    #[test]
    fn contains_expected_components() {
        let p = config_path().expect("home_dir should be resolvable in test env");
        let comps: Vec<String> = p
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert!(
            comps.iter().any(|c| c == ".config"),
            "missing .config component: {:?}",
            comps
        );
        assert!(
            comps.iter().any(|c| c == "ccli"),
            "missing ccli component: {:?}",
            comps
        );
        assert!(
            comps.iter().any(|c| c == "config.toml"),
            "missing config.toml component: {:?}",
            comps
        );
    }
}
