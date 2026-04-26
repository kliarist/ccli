//! Top-level CLI parser and command dispatch types.
//!
//! Global output flags (`--plain`, `--no-headers`, `--columns`) are defined here
//! with `global = true` so every subcommand inherits them automatically (D-06, D-07,
//! RESEARCH.md Pattern 1). Plan 05 adds the `init` submodule.

use clap::{Parser, Subcommand};

use crate::output::{parse_columns, OutputConfig};

/// `ccli` — Confluence Data Center CLI.
#[derive(Parser, Debug)]
#[command(
    name = "ccli",
    about = "Confluence Data Center CLI",
    version,
)]
pub struct Cli {
    /// Output tab-separated values (no box-drawing characters).
    /// When stdout is piped, TSV is the default even without this flag.
    #[arg(long, global = true)]
    pub plain: bool,

    /// Suppress the column header row in tabular output.
    #[arg(long, global = true)]
    pub no_headers: bool,

    /// Comma-separated columns to include, in order.
    /// Example: --columns key,name,status
    #[arg(long, global = true, value_name = "COLS")]
    pub columns: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configure Confluence URL and Personal Access Token interactively.
    Init,
    // Phase 2 will add: Space(SpaceArgs)
    // Phase 3 will add: Page(PageArgs), Blog(BlogArgs)
    // Phase 4 will add: Comment(...), Attachment(...)
}

impl Cli {
    /// Build an OutputConfig from the parsed global flags.
    /// Called by command handlers that emit tabular output.
    pub fn output_config(&self) -> OutputConfig {
        OutputConfig {
            plain: self.plain,
            no_headers: self.no_headers,
            columns: self.columns.as_deref().map(parse_columns),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_init_subcommand() {
        let cli = Cli::try_parse_from(["ccli", "init"]).expect("parse");
        assert!(matches!(cli.command, Commands::Init));
        assert!(!cli.plain);
        assert!(!cli.no_headers);
        assert!(cli.columns.is_none());
    }

    #[test]
    fn plain_flag_before_subcommand() {
        let cli = Cli::try_parse_from(["ccli", "--plain", "init"]).expect("parse");
        assert!(cli.plain);
    }

    #[test]
    fn plain_flag_after_subcommand() {
        // global = true means clap accepts the flag in either position.
        let cli = Cli::try_parse_from(["ccli", "init", "--plain"]).expect("parse");
        assert!(cli.plain);
    }

    #[test]
    fn no_headers_flag() {
        let cli = Cli::try_parse_from(["ccli", "init", "--no-headers"]).expect("parse");
        assert!(cli.no_headers);
    }

    #[test]
    fn columns_flag_captures_string() {
        let cli = Cli::try_parse_from(["ccli", "init", "--columns", "key,name"]).expect("parse");
        assert_eq!(cli.columns.as_deref(), Some("key,name"));
    }

    #[test]
    fn missing_subcommand_errors() {
        let result = Cli::try_parse_from(["ccli"]);
        assert!(result.is_err(), "clap should require a subcommand");
    }

    #[test]
    fn output_config_passes_through_flags() {
        let cli = Cli::try_parse_from([
            "ccli", "--plain", "--no-headers", "--columns", "key, name", "init",
        ]).expect("parse");
        let cfg = cli.output_config();
        assert!(cfg.plain);
        assert!(cfg.no_headers);
        assert_eq!(cfg.columns.as_deref(), Some(&["key".to_string(), "name".to_string()][..]));
    }

    #[test]
    fn output_config_columns_none_when_flag_absent() {
        let cli = Cli::try_parse_from(["ccli", "init"]).expect("parse");
        let cfg = cli.output_config();
        assert!(cfg.columns.is_none());
    }
}
