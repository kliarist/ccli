//! TUI shell — terminal lifecycle, event loop, and top-level dispatch.
//!
//! Locked decisions implemented:
//! - D-10: bare `ccli` launches this TUI on the Spaces browse view
//! - D-12: `q` always quits; `Esc` closes overlays (filter, modal)
//! - D-13: static help bar + `?` modal

pub mod app;

pub async fn run() -> anyhow::Result<()> {
    // Stub: replaced in Plan 05 (02-05-PLAN.md)
    anyhow::bail!("TUI not yet implemented — run 'ccli space list' for non-interactive mode")
}
