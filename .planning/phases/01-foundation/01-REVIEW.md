---
phase: 01-foundation
reviewed: 2026-04-26T00:00:00Z
depth: standard
files_reviewed: 14
files_reviewed_list:
  - .gitignore
  - Cargo.lock
  - Cargo.toml
  - src/api/client.rs
  - src/api/error.rs
  - src/api/mod.rs
  - src/cli/init.rs
  - src/cli/mod.rs
  - src/config/mod.rs
  - src/config/path.rs
  - src/main.rs
  - src/output/mod.rs
  - src/output/table.rs
  - src/output/tsv.rs
findings:
  critical: 0
  warning: 4
  info: 2
  total: 6
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-04-26T00:00:00Z
**Depth:** standard
**Files Reviewed:** 14
**Status:** issues_found

## Summary

All fourteen source files from the Phase 1 foundation were reviewed. The overall quality is high: error categorisation, atomic config writes, PAT masking, TLS handling, and TTY dispatch are all implemented cleanly with good test coverage. No security vulnerabilities or data-loss bugs were found.

Four warnings were identified: two are latent panics triggered by out-of-bounds row access in the output renderers; one is a correctness defect in TSV output when cell values contain tab characters; and one is an incomplete cleanup path for a temp file on a failed rename. Two informational items cover a fragile byte-slice assumption in `mask_pat_hint` and minimal URL validation.

---

## Warnings

### WR-01: Panic on out-of-bounds row index in table renderer

**File:** `src/output/table.rs:37`
**Issue:** `row[i].as_str()` uses a raw index from `col_indices` without bounds-checking the row. `col_indices` is derived from the header length via `resolve_columns`, so it is correct for well-formed rows. However, if a caller passes rows with fewer columns than headers (a realistic mistake in Phase 2+ command handlers), this panics at runtime instead of producing a recoverable error or a blank cell.

The same pattern is present in `src/output/tsv.rs:41`.

**Fix:** Add a bounds check or use `.get(i)`:
```rust
// table.rs render_string — replace the map in the row loop:
let filtered: Vec<&str> = col_indices.iter()
    .map(|&i| row.get(i).map(|s| s.as_str()).unwrap_or(""))
    .collect();

// tsv.rs render_to — same replacement:
let filtered: Vec<&str> = col_indices.iter()
    .map(|&i| row.get(i).map(|s| s.as_str()).unwrap_or(""))
    .collect();
```

---

### WR-02: Unescaped tab/newline characters in TSV cell values break pipeline consumers

**File:** `src/output/tsv.rs:42`
**Issue:** Cell values are written with `filtered.join("\t")` and `writeln!` without escaping embedded tab (`\t`) or newline (`\n`, `\r`) characters. A Confluence space or page whose name or description contains a tab will silently produce a structurally invalid TSV line, causing `awk -F'\t'`, `cut -f`, and `sort -t$'\t'` pipelines — explicitly called out in the module doc comment as the supported use case — to misparse the output.

**Fix:** Escape (or strip) control characters before joining. A minimal approach that preserves readability:
```rust
fn escape_tsv_cell(s: &str) -> String {
    s.replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r")
}

// In render_to:
let filtered: Vec<String> = col_indices.iter()
    .map(|&i| escape_tsv_cell(row.get(i).map(|s| s.as_str()).unwrap_or("")))
    .collect();
writeln!(writer, "{}", filtered.join("\t"))?;
```

Alternatively, replace with a space, depending on the desired contract (document the choice).

---

### WR-03: Temp file left on disk with plaintext token if rename fails

**File:** `src/config/mod.rs:85-86`
**Issue:** `save_to` writes the config (including the PAT) to `<path>.toml.tmp`, then renames it. If `fs::rename` fails — for example due to a cross-device move or a permissions error on the destination directory — the function returns an error via `?` but the `.toml.tmp` file remains on disk. That file was written without the `0o600` permission hardening applied to the final path (the `chmod` runs only after a successful rename). On a shared or multi-user system this is a brief window where the token is world-readable.

**Fix:** Remove the temp file in an error path, and set permissions on the temp file before rename:
```rust
pub(crate) fn save_to(path: &Path, config: &Config) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, &content)?;

    // Restrict permissions on the temp file BEFORE rename so the token
    // is never briefly world-readable if rename fails.
    #[cfg(unix)]
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;

    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp); // best-effort cleanup
        return Err(e.into());
    }
    Ok(())
}
```

---

### WR-04: `std::env::set_var` / `remove_var` unsafe in multi-threaded test harness

**File:** `src/api/client.rs:153`, `src/api/client.rs:164-165`, `src/api/client.rs:172-174`
**Also:** `src/config/mod.rs:145-146`, `src/config/mod.rs:214`, `src/config/mod.rs:229`
**Issue:** `std::env::set_var` and `std::env::remove_var` are unsound in a multi-threaded process (Rust 1.83 made them `unsafe`). The `ENV_LOCK` mutex serialises these specific tests against each other, but does not prevent other test threads in the same binary from calling `std::env::var("CCLI_INSECURE")` or `std::env::var("CCLI_URL")` concurrently. Cargo runs all tests in a single process by default, and `client.rs` tests that hit `read_insecure_env()` (called from `Client::new` and `test_connection`) interleave with env mutations in config tests.

**Fix:** Acquire the same `ENV_LOCK` (or a shared crate-level lock) in every test that reads or writes these env vars. For a more robust solution, pass the env state as a function parameter rather than reading it globally:
```rust
// Alternative: make read_insecure_env injectable for tests
fn build_client_inner(token: &str, accept_invalid_certs: bool) -> Result<ReqwestClient, AppError> { ... }

// In tests, call build_client_inner directly with a known bool
// rather than mutating the environment.
```

---

## Info

### IN-01: Byte-index slicing on a `chars().count()`-guarded string in `mask_pat_hint`

**File:** `src/cli/init.rs:117-118`
**Issue:** `mask_pat_hint` guards the slice with `token.chars().count()` (Unicode-correct), but then slices with `&token[..2]` and `&token[token.len() - 3..]` (byte indices). The comment acknowledges this with "PATs are ASCII, so byte indexing aligns with char indexing." This is a safe assumption for Confluence PATs today, but the function signature accepts any `&str` and there is no assertion or compile-time enforcement of the ASCII constraint. A multi-byte character in the first two or last three positions would cause a panic in a debug build and undefined slicing behaviour in a release build.

**Fix:** Use `chars` explicitly, or add a debug assertion:
```rust
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
```

---

### IN-02: `validate_url` accepts bare scheme with no host

**File:** `src/cli/init.rs:91-97`
**Issue:** `validate_url` only checks that the input starts with `http://` or `https://`. It accepts `http://` (scheme only, no host), `https:// ` (whitespace host), and arbitrarily long strings with a valid prefix. The user will receive a confusing network error from `test_connection` rather than a clear prompt-level validation message. The comment acknowledges this as a Phase 1 limitation, so this is informational.

**Fix (Phase 2+ candidate):** Require at least one character after the scheme separator and trim whitespace:
```rust
pub(crate) fn validate_url(s: &str) -> Result<(), &'static str> {
    let rest = s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .ok_or("URL must start with http:// or https://")?;
    if rest.trim().is_empty() {
        return Err("URL must include a host (e.g. https://confluence.example.com)");
    }
    Ok(())
}
```

---

_Reviewed: 2026-04-26T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
