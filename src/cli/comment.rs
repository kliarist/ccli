//! `ccli comment` subcommand handlers (stub — implementation in Plan 04).
//!
//! This stub exists so Plan 02 can wire `Commands::Comment` dispatch in main.rs
//! while Plan 04 fills in the actual handler logic. DO NOT add real logic here —
//! it belongs in Plan 04.

use crate::cli::{Cli, CommentArgs};

pub async fn run(_cli: &Cli, _args: &CommentArgs) -> anyhow::Result<()> {
    anyhow::bail!("ccli comment is not yet implemented (Plan 04)")
}
