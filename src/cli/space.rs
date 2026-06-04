//! `ccli space list` non-interactive handler.

use anyhow::Context;

use crate::api::space::Space;
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
    formatter.print(headers, &build_rows(&spaces));

    Ok(())
}

pub(crate) fn build_rows(spaces: &[Space]) -> Vec<Vec<String>> {
    spaces
        .iter()
        .map(|s| {
            vec![
                s.key.clone(),
                s.name.clone(),
                s.space_type.clone().unwrap_or_default(),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::space::{Space, SpaceLinks};

    fn make_space(key: &str, name: &str, space_type: Option<&str>) -> Space {
        Space {
            id: 1,
            key: key.to_string(),
            name: name.to_string(),
            space_type: space_type.map(ToString::to_string),
            links: SpaceLinks {
                webui: Some(format!("/display/{}", key)),
                ..Default::default()
            },
        }
    }

    #[test]
    fn build_rows_has_3_columns_per_space() {
        let spaces = vec![make_space("DEV", "Development", Some("global"))];
        let rows = build_rows(&spaces);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], vec!["DEV", "Development", "global"]);
    }

    #[test]
    fn build_rows_null_type_uses_empty_string() {
        let spaces = vec![make_space("ARCH", "Archive", None)];
        let rows = build_rows(&spaces);
        assert_eq!(
            rows[0][2], "",
            "null space_type should produce empty string"
        );
    }

    #[test]
    fn build_rows_preserves_order() {
        let spaces = vec![
            make_space("AAA", "Alpha", Some("global")),
            make_space("ZZZ", "Zeta", Some("personal")),
        ];
        let rows = build_rows(&spaces);
        assert_eq!(rows[0][0], "AAA");
        assert_eq!(rows[1][0], "ZZZ");
    }
}
