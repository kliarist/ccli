# Phase 5: 9.2.13 Compatibility Audit & Fixes - Pattern Map

**Mapped:** 2026-05-08
**Files analyzed:** 6 files to be modified + 1 new documentation file
**Analogs found:** 6 / 6 (all target files are the analogs — this is an in-place fix phase)

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/api/space.rs` | service | request-response (CRUD read) | self — existing file is the target | exact |
| `src/api/page.rs` | service | request-response (CRUD) | self — existing file is the target | exact |
| `src/api/comment.rs` | service | request-response (CRUD) | `src/api/space.rs` (same httpmock pattern) | exact |
| `src/api/attachment.rs` | service | request-response + file-I/O | `src/api/page.rs` (same httpmock pattern) | exact |
| `src/cli/page.rs` | controller | request-response | `src/api/space.rs` (same httpmock pattern in #[cfg(test)]) | exact |
| `COMPAT-9213.md` | documentation | N/A | no analog (new public artifact) | no analog |

> Note: Phase 5 is an in-place audit and fix phase. Every code target is already in the codebase.
> "Closest analog" for test patterns is the same module; for COMPAT-9213.md there is no code analog.

---

## Pattern Assignments

### `src/api/space.rs` (service, request-response — COMPAT-01)

**Analog:** self (lines read from the existing file)

**Imports pattern** (lines 14–18):
```rust
use serde::Deserialize;
use tracing::{debug, instrument};

use crate::api::client::Client;
use crate::api::error::AppError;
```

**Serde Option fields — defensive parsing pattern** (lines 34–38, 68–77):
```rust
// All pagination sentinel fields and optional detail fields already use Option<T>.
// When a 9.2.13 response omits a field that 9.2.19 includes, serde yields None — no parse failure.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpaceListLinks {
    pub next: Option<String>, // present when more pages exist (pagination)
    pub base: Option<String>,
}

// Nested Option chain for description.plain — handles null, {}, or absent:
pub description: Option<SpaceDescription>,   // SpaceDetail line 68
// struct SpaceDescription { pub plain: Option<PlainBody>, }  // line 75-77
// struct PlainBody { pub value: Option<String>, ... }        // line 81-84
```

**Pagination loop pattern** (lines 103–155):
```rust
// Stop when _links.next is absent OR size < limit (protects against empty-string sentinel)
let has_next = page.links.next.is_some();
all.extend(page.results);
if !has_next || fetched < limit {
    break;
}
start += limit;
```

**Error handling pattern** (lines 125–150):
```rust
match resp.status().as_u16() {
    200 => { /* parse + paginate */ }
    401 | 403 => {
        return Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        ))
    }
    status => {
        return Err(AppError::Api(format!(
            "Unexpected HTTP {} from /rest/api/space",
            status
        )))
    }
}
```

**Network error mapping pattern** (lines 117–124):
```rust
.map_err(|e| {
    if e.is_connect() || e.is_timeout() {
        AppError::Network(format!("Cannot reach server: {}", e))
    } else {
        AppError::Network(e.to_string())
    }
})?;
```

**httpmock unit test pattern** (lines 204–216):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use httpmock::prelude::*;

    fn test_client(base_url: &str) -> Client {
        Client::new(&Config {
            url: base_url.to_string(),
            token: "AT-test-token".to_string(),
        })
        .expect("client")
    }

    #[tokio::test]
    async fn <test_name_describing_9213_shape>() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/space");
            then.status(200).json_body(serde_json::json!({ /* 9.2.13 shape */ }));
        });
        let client = test_client(&server.base_url());
        let result = list_all_spaces(&client).await.expect("ok");
        // assertions
    }
}
```

**Known risk areas for COMPAT-01 (from RESEARCH.md):**
- `description.plain` on a space with no description: may be `{}`, `null`, or absent. All produce `None` via the nested `Option` chain — no code change likely needed, but verify the UI renders the None case gracefully.
- `_links.next` sentinel: if 9.2.13 returns `""` instead of absent/null, the `is_some()` guard is true — add `&& !next.is_empty()` defensively if a pagination hang is observed.

---

### `src/api/page.rs` (service, request-response CRUD — COMPAT-02 / COMPAT-03)

**Analog:** self

**Imports pattern** (lines 16–20): identical to `space.rs` — same four imports.

**ContentType enum pattern** (lines 26–40):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType { Page, BlogPost }

impl ContentType {
    pub fn as_api_str(&self) -> &'static str {
        match self {
            ContentType::Page => "page",
            ContentType::BlogPost => "blogpost",
        }
    }
}
```

**query() builder pattern — WR-05 injection protection** (lines 160–168):
```rust
let resp = client
    .inner()
    .get(&base)
    .query(&[
        ("spaceKey", space_key),
        ("type", content_type.as_api_str()),
        ("start", &start.to_string()),
        ("limit", &limit.to_string()),
        ("expand", "version,ancestors"),
    ])
    .send()
    .await
    .map_err(/* network error map */)?;
```

**Serde struct fields pattern** (lines 61–128):
```rust
// All optional/expandable fields use Option<T> — handles missing expand= fields from 9.2.13
pub version: Option<PageVersion>,
pub ancestors: Option<Vec<PageAncestor>>,
pub body: Option<PageBody>,       // PageBody.storage: Option<StorageBody>
                                  // StorageBody.value: Option<String>
```

**409 Conflict error pattern** (lines 380–383):
```rust
409 => Err(AppError::Api(format!(
    "Conflict: page was updated by someone else (version {}).",
    current_version
))),
```

**New test for 9.2.13 fix — copy from lines 412–435 (list sorted test):**
```rust
#[tokio::test]
async fn list_all_pages_returns_sorted_vec_on_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/rest/api/content")
            .query_param("type", "page")
            .query_param("spaceKey", "DEV");
        then.status(200).json_body(serde_json::json!({
            "results": [ /* use 9.2.13-compatible shape here */ ],
            "start": 0, "limit": 50, "size": 2, "_links": {}
        }));
    });
    // ...
}
```

**Known risk areas for COMPAT-02/03:**
- `ancestors` may be absent or `[]` on older instances — already `Option<Vec<PageAncestor>>`, safe.
- `expand=version,ancestors` field population: if `version` is not returned for list results on 9.2.13, the `p.version.as_ref().and_then(...)` chain in `cli/page.rs` line 89 produces empty string (not a crash).

---

### `src/api/comment.rs` (service, request-response — COMPAT-04)

**Analog:** self (pattern mirrors `space.rs` and `page.rs` exactly)

**Imports pattern** (lines 11–14): same four imports as other api modules.

**CommentListResponse — minimal struct, no pagination** (lines 64–67):
```rust
#[derive(Debug, Clone, Deserialize)]
struct CommentListResponse {
    pub results: Vec<Comment>,
}
// Note: no start/limit/size fields — serde ignores extra fields from actual response (no deny_unknown_fields)
```

**httpmock test pattern** (lines 158–170): identical `test_client` helper as all other modules.

**Known risk areas for COMPAT-04:**
- Low probability of breakage. The `CommentListResponse` struct only requires `results` — extra envelope fields (`start`, `limit`, `size`) from the server are silently ignored by serde.
- If `body.storage` is absent in the 9.2.13 response, the nested `Option<CommentBody>` / `Option<StorageBody>` chain produces `None` safely.

---

### `src/api/attachment.rs` (service, request-response + file-I/O — COMPAT-05)

**Analog:** self

**Imports pattern** (lines 12–20):
```rust
use std::path::Path;
use bytes::Bytes;
use reqwest::multipart;
use serde::Deserialize;
use tracing::instrument;
use crate::api::client::Client;
use crate::api::error::AppError;
```

**X-Atlassian-Token header — REQUIRED for DC attachment upload** (lines 187–192):
```rust
let resp = client
    .inner()
    .post(&url)
    .header("X-Atlassian-Token", "no-check") // REQUIRED for DC attachment upload
    .multipart(form)
    .send()
    .await
    .map_err(/* network error map */)?;
```

**AttachmentExtensions camelCase field names** (lines 36–40):
```rust
pub struct AttachmentExtensions {
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    #[serde(rename = "fileSize")]
    pub file_size: Option<u64>,
}
```

**download_path resolution pattern** (lines 127–135):
```rust
// download path may be absolute path on server (starts with /); resolve against base_url.
let download_url = if download_path.starts_with("http") {
    download_path.to_string()
} else {
    format!(
        "{}{}",
        client.base_url().trim_end_matches('/'),
        download_path
    )
};
```

**httpmock test with multipart header assertion** (lines 331–346):
```rust
#[tokio::test]
async fn add_attachment_sends_multipart_with_x_atlassian_token_header() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/rest/api/content/123/child/attachment")
            .header("X-Atlassian-Token", "no-check");
        then.status(200);
    });
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    std::fs::write(tmp.path(), b"test content").expect("write");
    let client = test_client(&server.base_url());
    let result = add_attachment(&client, "123", tmp.path()).await;
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    m.assert();
}
```

**Known risk areas for COMPAT-05:**
- `X-Atlassian-Token: no-check` is already correctly implemented — do NOT change it.
- If 9.2.13 returns 403 on upload, read the response body carefully before assuming the header is wrong.
- `AttachmentExtensions.mediaType`/`fileSize` expected stable; if list shows empty MIME column, the field name changed — inspect with curl.

---

### `src/cli/page.rs` (controller, request-response — COMPAT-06 CQL Search)

**Analog:** self (lines 302–409 contain the search logic)

**Inline SearchResponse structs pattern** (lines 304–327):
```rust
// These structs are defined locally (not in api/page.rs) because the search endpoint
// has a different response shape from the content list endpoint.
#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResult {
    id: String,
    title: String,
    #[serde(default)]
    space: Option<SearchResultSpace>,   // Option protects against absent/empty space object
    #[serde(rename = "_links", default)]
    links: SearchResultLinks,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResultSpace {
    key: String,  // NON-optional — if object is present but key is absent, parse fails
}

#[derive(Debug, serde::Deserialize, Default)]
struct SearchResultLinks {
    webui: Option<String>,
}
```

**RESEARCH.md open question addressed in code — SearchResultSpace.key:**
If 9.2.13 returns `"space": {}` (empty object, key absent), `SearchResultSpace { key: String }` will parse-fail.
Fix pattern if needed:
```rust
// BEFORE:
struct SearchResultSpace { key: String }
// AFTER (defensive):
struct SearchResultSpace {
    #[serde(default)]
    key: Option<String>,
}
// And update call site: r.space.as_ref().and_then(|s| s.key.as_deref()).unwrap_or("")
```

**httpmock test helper for search** (lines 438–510):
```rust
// cli/page.rs uses a local run_search() helper in tests (not a #[cfg(test)] test_client only).
// New tests for 9.2.13 fixes follow the same run_search() pattern.
fn test_client(base_url: &str) -> Client {
    Client::new(&Config {
        url: base_url.to_string(),
        token: "AT-test-token".to_string(),
    })
    .expect("client")
}

async fn run_search(client: &Client, cql: &str, limit: u32) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)> {
    // ... duplicates handle_search HTTP logic without config load
}
```

**Known risk areas for COMPAT-06:**
- `SearchResultSpace.key` is non-Optional — highest fix probability in the search module (see fix pattern above).
- Extra top-level fields in the search response (`totalSize`, `cqlQuery`, `searchDuration`) are silently ignored by serde since `deny_unknown_fields` is not set.

---

### `COMPAT-9213.md` (documentation artifact — TMAT-01)

**Analog:** None — new public-facing file at repo root.

**Required table format (D-61):**
```markdown
| Endpoint Area | Expected Behavior | Observed on 9.2.13 | Status | Notes |
|---------------|-------------------|-------------------|--------|-------|
| Spaces        | ...               | ...               | PASS   | ...   |
| Pages         | ...               | ...               | FIXED  | ...   |
| Blog Posts    | ...               | ...               | PASS   | ...   |
| Comments      | ...               | ...               | PASS   | ...   |
| Attachments   | ...               | ...               | PASS   | ...   |
| CQL Search    | ...               | ...               | PASS   | ...   |
```

Status values: `PASS` (worked as-is), `FIXED` (code change was needed), `FAIL` (broken, no fix applied — should not occur).

**Tone:** factual, concise, written for external Confluence DC CLI users. No internal planning jargon.
**Location:** repo root alongside `CHANGELOG.md`.
**Written:** all at once AFTER all 6 areas are tested and fixed (D-60).

---

## Shared Patterns

### Authentication — PAT via default header (all api modules)

**Source:** `src/api/client.rs` (referenced in RESEARCH.md lines 377–381)
```rust
// PAT is injected as a default header on reqwest::Client at construction time.
// Every subsequent request carries it automatically — no per-request auth needed.
let auth_value = header::HeaderValue::from_str(&format!("Bearer {}", token))?;
headers.insert(header::AUTHORIZATION, auth_value);
```
**Apply to:** All audit areas. The PAT format is stable across 9.2.x — do NOT change auth logic.

### Error Taxonomy (all api modules)

**Source:** `src/api/error.rs` (lines 4–16)
```rust
pub enum AppError {
    Auth(String),     // 401 / 403 responses
    Network(String),  // connect/timeout failures
    Config(String),   // config load failures
    Api(String),      // all other API errors including parse failures
}
```
**Apply to:** All fixes in all api modules. No new error variants needed.

### Network Error Mapping (all api modules)

**Source:** `src/api/space.rs` lines 117–124 (identical in all four api modules)
```rust
.map_err(|e| {
    if e.is_connect() || e.is_timeout() {
        AppError::Network(format!("Cannot reach server: {}", e))
    } else {
        AppError::Network(e.to_string())
    }
})?;
```
**Apply to:** All HTTP `.send().await` call sites. Copy verbatim.

### Serde Option Fields for Missing/Null JSON Fields (all api modules)

**Source:** `src/api/space.rs` lines 34–38 and `src/api/page.rs` lines 67–70
```rust
// When 9.2.13 omits a field that 9.2.19 includes, add Option<T> + #[serde(default)].
// serde deserializes absent/null as None — no parse failure.
#[serde(default)]
pub some_field: Option<SomeType>,
```
**Apply to:** Any struct field that a live 9.2.13 response omits.

### D-67 Fix-and-Update-Existing-Tests Rule (all modules)

When a serde struct field type changes (e.g., `String` → `Option<String>`), **update ALL existing mock
responses** in the module's `#[cfg(test)]` block to match the new shape. Do NOT add a separate
`tests_9213` module. Goal: one unified response shape that compiles and passes for both versions.

```rust
// BAD: separate 9.2.13 module
#[cfg(test)] mod tests_9213 { ... }

// GOOD: update existing mock JSON in the existing tests block to use the compatible shape,
//       then add a new test that specifically covers the behavior that was broken on 9.2.13.
```

### Pagination Loop Guard (space.rs, page.rs)

**Source:** `src/api/space.rs` lines 132–137
```rust
let has_next = page.links.next.is_some();
all.extend(page.results);
if !has_next || fetched < limit {
    break;
}
```
If 9.2.13 returns `"_links": {"next": ""}` (empty string), `is_some()` is `true` and the loop continues.
Defensive fix pattern if a pagination hang is observed:
```rust
let has_next = page.links.next.as_deref().map(|s| !s.is_empty()).unwrap_or(false);
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `COMPAT-9213.md` | documentation | N/A | New public-facing markdown artifact; no existing compatibility matrix files in the repo |

---

## Metadata

**Analog search scope:** `src/api/`, `src/cli/`
**Files read:** `src/api/space.rs`, `src/api/page.rs`, `src/api/comment.rs`, `src/api/attachment.rs`, `src/api/error.rs`, `src/cli/page.rs`
**Pattern extraction date:** 2026-05-08
