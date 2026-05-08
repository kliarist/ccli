# COMPAT-04: Comments — Findings

**Tested:** 2026-05-08
**Instance:** Confluence 9.2.13 (static analysis — no live instance available)
**Status:** PASS (UNVERIFIED — no live instance)

## Expected Behavior

`GET /rest/api/content/{id}/child/comment?expand=body.storage,version&limit=50` returns a
`{"results": [...]}` envelope of Comment objects. `POST` to the same path with
`{"type": "comment", "body": {"storage": {"value": "<xml>", "representation": "storage"}}}`
adds a comment and returns 200 or 201.

## Sub-area Status

| Sub-area | Status | Notes |
| -------- | ------ | ----- |
| List (Step A) | PASS (UNVERIFIED) | Static analysis: CommentListResponse uses `results: Vec<Comment>`; all struct fields except `id` and `title` are `Option<T>`. Existing unit test `list_comments_returns_vec_on_200` exercises the 9.2.13-shaped envelope (with extra `start`/`limit`/`size`/`_links` top-level keys, which serde ignores). No structural changes required. |
| Add (Step B) | PASS (UNVERIFIED) | Static analysis: `add_comment` sends `{"type":"comment","body":{"storage":{"value":...,"representation":"storage"}}}`. This matches the documented DC REST API v2 shape for adding a comment. Existing test `add_comment_posts_storage_xml_body` validates 200 response. 201 is also handled (200 \| 201 match arm). |
| Visible after add (Step C) | PASS (UNVERIFIED) | Not testable without live instance. Static analysis shows no caching or optimistic-update logic that would hide a newly posted comment from a subsequent list call. |

## Observed on 9.2.13

No live observation — checkpoint outcome pre-resolved as PASS due to unavailability of a live
Confluence 9.2.13 instance. Findings are based on static code analysis of `src/api/comment.rs`
against the Confluence DC REST API documentation for the 9.x series.

Key static findings supporting PASS:

1. `CommentListResponse` has no `#[serde(deny_unknown_fields)]`, so extra envelope keys
   (`start`, `limit`, `size`, `_links`) returned by 9.2.13 are silently ignored — this is
   already covered by the existing unit test JSON fixture which includes those extra keys.

2. `Comment.id` and `Comment.title` are the only non-Optional fields. Both are guaranteed
   present in any valid Confluence comment response per the API contract.

3. `Comment.body` (`Option<CommentBody>`) and its nested `storage` (`Option<StorageBody>`)
   handle the case where `body` or `body.storage` is absent — correct defensive posture per
   T-05-14 mitigation.

4. `add_comment` handles both 200 and 201 status codes (`200 | 201 => Ok(())`), covering
   Confluence DC instances that return 201 on comment creation.

5. The plain-text-to-storage-XML wrapping contract (Phase 4, D-51/D-52) is correctly
   implemented in `src/cli/comment.rs`; `add_comment` receives pre-formatted storage XML.

## Fix Applied

None — Status is PASS. `src/api/comment.rs` is unchanged.

## Notes for COMPAT-9213.md

COMPAT-04 (Comments): Static analysis confirms no structural changes needed for 9.2.13
compatibility. The existing Comment struct's fully-Optional field design (all fields except
`id`/`title` are `Option<T>`) and the absence of `deny_unknown_fields` provide adequate
defensive parsing against minor envelope variations in the 9.x patch range.
