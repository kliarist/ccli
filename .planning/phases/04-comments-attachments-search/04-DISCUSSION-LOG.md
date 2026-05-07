# Phase 4: Comments, Attachments & Search - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-07
**Phase:** 04-comments-attachments-search
**Areas discussed:** CQL search output, Comments TUI depth, Comment creation format, Attachment download path

---

## CQL Search Output

### Q1: Search output mode

| Option | Description | Selected |
|--------|-------------|----------|
| Always table | One-shot query result → always stdout table, composable with pipes | ✓ |
| TUI when TTY (D-35 consistent) | Browseable results screen with list + preview pane when interactive | |
| You decide | Let Claude pick based on existing patterns | |

**User's choice:** Always table  
**Notes:** Search is a query, not a live list — explicit exception to D-35.

---

### Q2: Result limit and columns

| Option | Description | Selected |
|--------|-------------|----------|
| 25 results, title + ID + space + URL | Most actionable columns; --limit flag available | ✓ |
| 50 results, title + ID + space | Matches existing page list limit, narrower table | |
| No default limit, all results | Walk all pages; can be slow for broad queries | |

**User's choice:** 25 results, title + ID + space + URL  
**Notes:** URL column makes results immediately actionable.

---

### Q3: CQL input method

| Option | Description | Selected |
|--------|-------------|----------|
| Positional arg | `ccli page search "<CQL>"` — concise, shell-friendly | ✓ |
| --query / -q flag | More explicit but verbose | |

**User's choice:** Positional arg

---

## Comments TUI Depth

### Q1: TUI integration depth

| Option | Description | Selected |
|--------|-------------|----------|
| Full CommentsBrowse screen | New Screen variant, `c` key on page list, list + preview pane | ✓ |
| Preview pane only | Comments in page preview pane, no new screen | |
| CLI only (no TUI) | `ccli comment list` table, doesn't satisfy CMNT-01 | |

**User's choice:** Full CommentsBrowse screen  
**Notes:** Same push/pop stack pattern as SpacesBrowse → PagesBrowse.

---

### Q2: Keybinding

| Option | Description | Selected |
|--------|-------------|----------|
| c key | Mnemonic for 'comments', available in existing key space | ✓ |
| Enter (drill-down) | D-32 already locked Enter = open in browser | |

**User's choice:** `c` key

---

### Q3: List pane content

| Option | Description | Selected |
|--------|-------------|----------|
| Author + date + first line | Truncated first line gives content hint; full body in preview | ✓ |
| Author + date only | Cleaner, but no content hint | |
| Comment count in page list | Simpler but doesn't satisfy CMNT-01 full intent | |

**User's choice:** Author + date + first line (truncated)

---

## Comment Creation Format

### Q1: Editor input format

| Option | Description | Selected |
|--------|-------------|----------|
| Plain text | CLI wraps in `<p>` blocks; less friction for prose comments | ✓ |
| Storage XML (D-40 consistent) | Same as page create; high friction for a quick comment | |

**User's choice:** Plain text  
**Notes:** Intentional divergence from D-40. Comments are prose; pages need structure.

---

### Q2: Multi-paragraph support

| Option | Description | Selected |
|--------|-------------|----------|
| Multi-paragraph (blank lines → `<p>` blocks) | Common convention, mirrors Markdown paragraphs | ✓ |
| Single paragraph only | Everything becomes one `<p>` block | |

**User's choice:** Multi-paragraph (blank lines split into separate `<p>` blocks)

---

## Attachment Download Path

### Q1: Download destination

| Option | Description | Selected |
|--------|-------------|----------|
| Attachment name + saves to CWD | `ccli attachment get 12345 design.png` → `./design.png`; --output flag available | ✓ |
| Attachment name + --output required | User always specifies destination | |
| `<FILENAME>` is local destination path | Flexible but ambiguous identifier vs destination | |

**User's choice:** Attachment name + saves to CWD (--output flag for override)

---

### Q2: Upload MIME type detection

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-detect from file extension | mime_guess crate; fallback to application/octet-stream | ✓ |
| Always application/octet-stream | Simple but breaks image/PDF inline rendering | |
| --content-type flag | Explicit but adds friction to every upload | |

**User's choice:** Auto-detect from file extension

---

### Q3: Attachment list columns

| Option | Description | Selected |
|--------|-------------|----------|
| Filename + size + media-type + date | 4 columns, human-readable size (KB/MB) | ✓ |
| Filename + size only | Minimal, fits narrow terminals | |
| Filename + size + author + date | Author instead of media-type | |

**User's choice:** Filename + size + media-type + date

---

## Claude's Discretion

- Comment REST endpoint: `POST /rest/api/content/{page_id}/child/comment`
- Attachment endpoints: child/attachment for list; multipart POST for upload; `_links.download` for download
- CommentsBrowse state: `Screen::CommentsBrowse { page_id: String }` in existing Screen enum
- Comment API fetch: `GET /rest/api/content/{id}/child/comment?expand=body.storage,version&limit=50`
- `mime_guess` crate for MIME detection on upload

## Deferred Ideas

- Browseable search results in TUI (v2)
- Inline/body comments vs page-level comments distinction (post-v1)
- Attachment version history (post-v1)
- Batch attachment download (post-v1)
- Saved CQL aliases — already v2 requirement SRCH-01
- Fuzzy filter on comment list in TUI (post-v1)
