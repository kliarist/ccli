//! `ccli space list` non-interactive handler.
//!
//! Locked decisions implemented:
//! - D-11: ccli space list ALWAYS prints a table — never opens the TUI.
//! - D-17: fetches every page of /rest/api/space (same walk as D-14 TUI pre-fetch).
//!
//! Output uses OutputFormatter so --plain, --no-headers, --columns work for free (OUT-01..04).

use anyhow::Context;

use crate::api::{list_all_spaces, Client};
use crate::cli::{Cli, SpaceArgs};
use crate::config;
use crate::output::OutputFormatter;

/// Handle `ccli space list` — fetch all spaces and print a formatted table.
pub async fn run(cli: &Cli, _args: &SpaceArgs) -> anyhow::Result<()> {
    let config = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;

    let client = Client::new(&config)?;
    let spaces = list_all_spaces(&client)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to fetch spaces")?;

    let formatter = OutputFormatter::new(cli.output_config());
    let headers = &["Key", "Name", "Type"];
    let rows: Vec<Vec<String>> = spaces
        .iter()
        .map(|s| {
            vec![
                s.key.clone(),
                s.name.clone(),
                s.space_type.clone().unwrap_or_default(),
            ]
        })
        .collect();
    formatter.print(headers, &rows);

    Ok(())
}
