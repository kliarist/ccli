//! Disk cache for Confluence spaces and pages lists.
//!
//! Cache files live in `~/.config/ccli/cache/`:
//!   - `spaces.json`           — full spaces list
//!   - `pages_{KEY}.json`      — page list per space key
//!
//! Each entry embeds the `base_url` so a config URL change is detected as a miss.
//! TTL is 5 minutes; stale entries are silently discarded and re-fetched.
//!
//! All I/O errors are swallowed — the cache is best-effort; callers must always
//! handle a None return from `load_*` by falling back to a network fetch.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::api::page::Page;
use crate::api::space::Space;

const TTL_SECS: u64 = 300; // 5 minutes

#[derive(Serialize, Deserialize)]
struct SpacesEntry {
    base_url: String,
    fetched_at: u64,
    spaces: Vec<Space>,
}

#[derive(Serialize, Deserialize)]
struct PagesEntry {
    base_url: String,
    fetched_at: u64,
    pages: Vec<Page>,
}

fn cache_dir() -> Option<PathBuf> {
    Some(home::home_dir()?.join(".config").join("ccli").join("cache"))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn is_fresh(fetched_at: u64) -> bool {
    unix_now().saturating_sub(fetched_at) < TTL_SECS
}

fn pages_filename(space_key: &str) -> String {
    let safe: String = space_key
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("pages_{}.json", safe)
}

// ─── Testable inner functions ──────────────────────────────────────────────────

fn load_spaces_from(dir: &Path, base_url: &str) -> Option<Vec<Space>> {
    let content = std::fs::read_to_string(dir.join("spaces.json")).ok()?;
    let entry: SpacesEntry = serde_json::from_str(&content).ok()?;
    if entry.base_url != base_url || !is_fresh(entry.fetched_at) {
        return None;
    }
    Some(entry.spaces)
}

fn save_spaces_to(dir: &Path, base_url: &str, spaces: &[Space]) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let entry = SpacesEntry {
        base_url: base_url.to_string(),
        fetched_at: unix_now(),
        spaces: spaces.to_vec(),
    };
    if let Ok(json) = serde_json::to_string(&entry) {
        let _ = std::fs::write(dir.join("spaces.json"), json);
    }
}

fn load_pages_from(dir: &Path, base_url: &str, space_key: &str) -> Option<Vec<Page>> {
    let content = std::fs::read_to_string(dir.join(pages_filename(space_key))).ok()?;
    let entry: PagesEntry = serde_json::from_str(&content).ok()?;
    if entry.base_url != base_url || !is_fresh(entry.fetched_at) {
        return None;
    }
    Some(entry.pages)
}

fn save_pages_to(dir: &Path, base_url: &str, space_key: &str, pages: &[Page]) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let entry = PagesEntry {
        base_url: base_url.to_string(),
        fetched_at: unix_now(),
        pages: pages.to_vec(),
    };
    if let Ok(json) = serde_json::to_string(&entry) {
        let _ = std::fs::write(dir.join(pages_filename(space_key)), json);
    }
}

// ─── Public API ────────────────────────────────────────────────────────────────

/// Return cached spaces if they are fresh and match `base_url`.
pub fn load_spaces(base_url: &str) -> Option<Vec<Space>> {
    load_spaces_from(&cache_dir()?, base_url)
}

/// Persist the spaces list to disk. Silently swallows I/O errors.
pub fn save_spaces(base_url: &str, spaces: &[Space]) {
    let Some(dir) = cache_dir() else { return };
    save_spaces_to(&dir, base_url, spaces);
}

/// Return cached pages for `space_key` if fresh and matching `base_url`.
pub fn load_pages(base_url: &str, space_key: &str) -> Option<Vec<Page>> {
    load_pages_from(&cache_dir()?, base_url, space_key)
}

/// Persist the pages list for `space_key` to disk. Silently swallows I/O errors.
pub fn save_pages(base_url: &str, space_key: &str, pages: &[Page]) {
    let Some(dir) = cache_dir() else { return };
    save_pages_to(&dir, base_url, space_key, pages);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::api::page::PageLinks;
    use crate::api::space::SpaceLinks;

    fn make_space(key: &str) -> Space {
        Space {
            id: 1,
            key: key.to_string(),
            name: format!("{} Space", key),
            space_type: Some("global".to_string()),
            links: SpaceLinks {
                webui: Some(format!("/display/{}", key)),
                self_url: None,
            },
        }
    }

    fn make_page(id: &str, title: &str) -> Page {
        Page {
            id: id.to_string(),
            title: title.to_string(),
            content_type: "page".to_string(),
            version: None,
            ancestors: None,
            links: PageLinks {
                webui: Some(format!("/x/{}", id)),
                self_url: None,
            },
        }
    }

    // ── pages_filename ────────────────────────────────────────────────────────

    #[test]
    fn pages_filename_alphanumeric_key_unchanged() {
        assert_eq!(pages_filename("DEV"), "pages_DEV.json");
    }

    #[test]
    fn pages_filename_hyphen_and_underscore_preserved() {
        assert_eq!(pages_filename("my-space_01"), "pages_my-space_01.json");
    }

    #[test]
    fn pages_filename_special_chars_replaced_with_underscore() {
        assert_eq!(pages_filename("my space!"), "pages_my_space_.json");
    }

    #[test]
    fn pages_filename_empty_key_produces_valid_filename() {
        assert_eq!(pages_filename(""), "pages_.json");
    }

    // ── is_fresh ──────────────────────────────────────────────────────────────

    #[test]
    fn is_fresh_returns_true_for_current_timestamp() {
        assert!(is_fresh(unix_now()));
    }

    #[test]
    fn is_fresh_returns_true_just_inside_ttl() {
        assert!(is_fresh(unix_now().saturating_sub(TTL_SECS - 1)));
    }

    #[test]
    fn is_fresh_returns_false_exactly_at_ttl_boundary() {
        assert!(!is_fresh(unix_now().saturating_sub(TTL_SECS)));
    }

    #[test]
    fn is_fresh_returns_false_well_past_ttl() {
        assert!(!is_fresh(unix_now().saturating_sub(TTL_SECS + 60)));
    }

    // ── spaces: save / load roundtrip ─────────────────────────────────────────

    #[test]
    fn save_and_load_spaces_roundtrip_preserves_keys_and_names() {
        let dir = TempDir::new().unwrap();
        let spaces = vec![make_space("DEV"), make_space("HR")];
        save_spaces_to(dir.path(), "https://example.com", &spaces);
        let loaded = load_spaces_from(dir.path(), "https://example.com").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].key, "DEV");
        assert_eq!(loaded[1].key, "HR");
    }

    #[test]
    fn load_spaces_returns_none_when_file_absent() {
        let dir = TempDir::new().unwrap();
        assert!(load_spaces_from(dir.path(), "https://example.com").is_none());
    }

    #[test]
    fn load_spaces_returns_none_when_base_url_differs() {
        let dir = TempDir::new().unwrap();
        save_spaces_to(
            dir.path(),
            "https://original.example.com",
            &[make_space("A")],
        );
        assert!(load_spaces_from(dir.path(), "https://other.example.com").is_none());
    }

    #[test]
    fn load_spaces_returns_none_when_entry_is_stale() {
        let dir = TempDir::new().unwrap();
        let entry = SpacesEntry {
            base_url: "https://example.com".to_string(),
            fetched_at: unix_now().saturating_sub(TTL_SECS + 1),
            spaces: vec![make_space("A")],
        };
        let json = serde_json::to_string(&entry).unwrap();
        std::fs::write(dir.path().join("spaces.json"), json).unwrap();
        assert!(load_spaces_from(dir.path(), "https://example.com").is_none());
    }

    #[test]
    fn load_spaces_returns_none_when_json_is_corrupt() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("spaces.json"), b"not json!!!").unwrap();
        assert!(load_spaces_from(dir.path(), "https://example.com").is_none());
    }

    #[test]
    fn save_spaces_overwrites_previous_entry() {
        let dir = TempDir::new().unwrap();
        save_spaces_to(dir.path(), "https://example.com", &[make_space("OLD")]);
        save_spaces_to(dir.path(), "https://example.com", &[make_space("NEW")]);
        let loaded = load_spaces_from(dir.path(), "https://example.com").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].key, "NEW");
    }

    #[test]
    fn save_spaces_with_empty_slice_roundtrips_to_empty_vec() {
        let dir = TempDir::new().unwrap();
        save_spaces_to(dir.path(), "https://example.com", &[]);
        let loaded = load_spaces_from(dir.path(), "https://example.com").unwrap();
        assert!(loaded.is_empty());
    }

    // ── pages: save / load roundtrip ──────────────────────────────────────────

    #[test]
    fn save_and_load_pages_roundtrip_preserves_ids_and_titles() {
        let dir = TempDir::new().unwrap();
        let pages = vec![make_page("1", "Alpha"), make_page("2", "Beta")];
        save_pages_to(dir.path(), "https://example.com", "DEV", &pages);
        let loaded = load_pages_from(dir.path(), "https://example.com", "DEV").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "1");
        assert_eq!(loaded[0].title, "Alpha");
        assert_eq!(loaded[1].title, "Beta");
    }

    #[test]
    fn load_pages_returns_none_when_file_absent() {
        let dir = TempDir::new().unwrap();
        assert!(load_pages_from(dir.path(), "https://example.com", "DEV").is_none());
    }

    #[test]
    fn load_pages_returns_none_when_base_url_differs() {
        let dir = TempDir::new().unwrap();
        save_pages_to(
            dir.path(),
            "https://original.example.com",
            "DEV",
            &[make_page("1", "T")],
        );
        assert!(load_pages_from(dir.path(), "https://other.example.com", "DEV").is_none());
    }

    #[test]
    fn load_pages_returns_none_when_entry_is_stale() {
        let dir = TempDir::new().unwrap();
        let entry = PagesEntry {
            base_url: "https://example.com".to_string(),
            fetched_at: unix_now().saturating_sub(TTL_SECS + 1),
            pages: vec![make_page("1", "T")],
        };
        let json = serde_json::to_string(&entry).unwrap();
        std::fs::write(dir.path().join("pages_DEV.json"), json).unwrap();
        assert!(load_pages_from(dir.path(), "https://example.com", "DEV").is_none());
    }

    #[test]
    fn load_pages_returns_none_when_json_is_corrupt() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("pages_DEV.json"), b"not json!!!").unwrap();
        assert!(load_pages_from(dir.path(), "https://example.com", "DEV").is_none());
    }

    #[test]
    fn pages_different_space_keys_write_to_separate_files() {
        let dir = TempDir::new().unwrap();
        save_pages_to(
            dir.path(),
            "https://example.com",
            "DEV",
            &[make_page("1", "Dev Page")],
        );
        save_pages_to(
            dir.path(),
            "https://example.com",
            "HR",
            &[make_page("2", "HR Page"), make_page("3", "Another")],
        );
        let dev = load_pages_from(dir.path(), "https://example.com", "DEV").unwrap();
        let hr = load_pages_from(dir.path(), "https://example.com", "HR").unwrap();
        assert_eq!(dev.len(), 1);
        assert_eq!(hr.len(), 2);
    }

    #[test]
    fn save_pages_with_empty_slice_roundtrips_to_empty_vec() {
        let dir = TempDir::new().unwrap();
        save_pages_to(dir.path(), "https://example.com", "DEV", &[]);
        let loaded = load_pages_from(dir.path(), "https://example.com", "DEV").unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn pages_cache_does_not_bleed_across_space_keys_with_similar_names() {
        let dir = TempDir::new().unwrap();
        save_pages_to(
            dir.path(),
            "https://example.com",
            "DEV",
            &[make_page("1", "Only DEV")],
        );
        // DEVOPS should be a completely separate file from DEV
        assert!(load_pages_from(dir.path(), "https://example.com", "DEVOPS").is_none());
    }
}
