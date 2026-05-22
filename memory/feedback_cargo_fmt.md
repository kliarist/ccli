---
name: feedback_cargo_fmt
description: Always run cargo fmt before committing — CI enforces it and local build/test do not
metadata:
  type: feedback
---

Always run `cargo fmt --all` before committing in this repo.

**Why:** CI runs `cargo fmt --all -- --check` and fails on any formatting diff. `cargo build` and `cargo test` do not enforce formatting, so it's easy to commit unformatted code and break CI.

**How to apply:** Pre-commit checklist: `cargo fmt --all && cargo clippy -- -D warnings && cargo test`. All three must pass before pushing.
