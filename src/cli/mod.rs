//! Top-level CLI parser and command dispatch types.
//!
//! Global output flags (`--plain`, `--no-headers`, `--columns`) are defined here
//! with `global = true` so every subcommand inherits them automatically (D-06, D-07,
//! RESEARCH.md Pattern 1). Plan 05 adds the `init` submodule.

use clap::{Parser, Subcommand};

use crate::output::{parse_columns, OutputConfig};

pub mod init;
pub mod space;
pub mod page;
pub mod blog;

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
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configure Confluence URL and Personal Access Token interactively.
    Init,
    /// List and browse Confluence spaces.
    Space(SpaceArgs),
    /// List, view, create, or edit Confluence pages (D-34..D-38, D-41).
    Page(PageArgs),
    /// List, view, create, or edit Confluence blog posts (D-37 — mirror of Page, no --parent on create).
    Blog(BlogArgs),
    // Phase 4 will add: Comment(...), Attachment(...)
}

/// Arguments for the `space` subcommand group.
#[derive(clap::Args, Debug)]
pub struct SpaceArgs {
    #[command(subcommand)]
    pub command: SpaceCommands,
}

/// Subcommands under `ccli space`.
#[derive(Subcommand, Debug)]
pub enum SpaceCommands {
    /// Print all accessible spaces as a formatted table.
    List,
}

/// Arguments for the `page` subcommand group (D-34..D-38, D-41).
#[derive(clap::Args, Debug)]
pub struct PageArgs {
    #[command(subcommand)]
    pub command: PageCommands,
}

/// Subcommands under `ccli page`.
#[derive(Subcommand, Debug)]
pub enum PageCommands {
    /// List pages in a space (table when piped — D-35).
    List {
        /// Space key (e.g. DEV) — required positional argument (D-34).
        space_key: String,
    },
    /// Print page content as plain text to stdout (D-36).
    View {
        /// Page ID (numeric string).
        page_id: String,
    },
    /// Create a new page (opens $EDITOR with storage XML template — D-38, D-40).
    Create {
        /// Space key. Prompted via dialoguer if omitted (D-38).
        #[arg(long)]
        space: Option<String>,
        /// Page title. Prompted via dialoguer if omitted (D-38).
        #[arg(long)]
        title: Option<String>,
        /// Optional parent page ID. Pages without --parent are created at space root.
        #[arg(long)]
        parent: Option<String>,
    },
    /// Edit an existing page's storage XML in $EDITOR (D-41).
    Edit {
        /// Page ID (numeric string).
        page_id: String,
    },
}

/// Arguments for the `blog` subcommand group (D-37 — mirror of Page).
#[derive(clap::Args, Debug)]
pub struct BlogArgs {
    #[command(subcommand)]
    pub command: BlogCommands,
}

/// Subcommands under `ccli blog`. Mirrors PageCommands; Create has NO `--parent` flag (Pitfall 5).
#[derive(Subcommand, Debug)]
pub enum BlogCommands {
    /// List blog posts in a space (table when piped — D-35).
    List {
        /// Space key (e.g. DEV) — required positional argument (D-34).
        space_key: String,
    },
    /// Print blog post content as plain text to stdout (D-36).
    View {
        /// Blog post ID (numeric string).
        post_id: String,
    },
    /// Create a new blog post (opens $EDITOR with storage XML template — D-38).
    /// No `--parent` flag — blog posts have no parent (Pitfall 5).
    Create {
        /// Space key. Prompted via dialoguer if omitted (D-38).
        #[arg(long)]
        space: Option<String>,
        /// Blog post title. Prompted via dialoguer if omitted (D-38).
        #[arg(long)]
        title: Option<String>,
    },
    /// Edit an existing blog post's storage XML in $EDITOR (D-41).
    Edit {
        /// Blog post ID (numeric string).
        post_id: String,
    },
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
        assert!(matches!(cli.command, Some(Commands::Init)));
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
    fn no_subcommand_launches_tui() {
        let result = Cli::try_parse_from(["ccli"]);
        assert!(result.is_ok(), "bare ccli should parse successfully — D-10 launches TUI");
        assert!(result.unwrap().command.is_none(), "command must be None when no subcommand given");
    }

    #[test]
    fn space_list_subcommand_parses() {
        let cli = Cli::try_parse_from(["ccli", "space", "list"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Space(_))));
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

    #[test]
    fn parses_page_list_with_space_key_positional() {
        let cli = Cli::try_parse_from(["ccli", "page", "list", "DEV"]).expect("parse");
        match cli.command {
            Some(Commands::Page(PageArgs { command: PageCommands::List { space_key } })) => {
                assert_eq!(space_key, "DEV");
            }
            other => panic!("expected Commands::Page(List), got {:?}", other),
        }
    }

    #[test]
    fn page_list_without_space_key_fails() {
        let result = Cli::try_parse_from(["ccli", "page", "list"]);
        assert!(result.is_err(), "D-34: space_key is required positional arg");
    }

    #[test]
    fn parses_page_view_with_page_id() {
        let cli = Cli::try_parse_from(["ccli", "page", "view", "12345"]).expect("parse");
        match cli.command {
            Some(Commands::Page(PageArgs { command: PageCommands::View { page_id } })) => {
                assert_eq!(page_id, "12345");
            }
            other => panic!("expected Commands::Page(View), got {:?}", other),
        }
    }

    #[test]
    fn parses_page_create_with_all_flags() {
        let cli = Cli::try_parse_from([
            "ccli", "page", "create",
            "--space", "DEV", "--title", "Hello", "--parent", "99",
        ]).expect("parse");
        match cli.command {
            Some(Commands::Page(PageArgs { command: PageCommands::Create { space, title, parent } })) => {
                assert_eq!(space.as_deref(), Some("DEV"));
                assert_eq!(title.as_deref(), Some("Hello"));
                assert_eq!(parent.as_deref(), Some("99"));
            }
            other => panic!("expected Commands::Page(Create), got {:?}", other),
        }
    }

    #[test]
    fn parses_page_create_with_no_flags_for_dialoguer_fallback() {
        let cli = Cli::try_parse_from(["ccli", "page", "create"]).expect("parse");
        match cli.command {
            Some(Commands::Page(PageArgs { command: PageCommands::Create { space, title, parent } })) => {
                assert!(space.is_none());
                assert!(title.is_none());
                assert!(parent.is_none());
            }
            other => panic!("expected Commands::Page(Create), got {:?}", other),
        }
    }

    #[test]
    fn parses_page_edit_with_page_id() {
        let cli = Cli::try_parse_from(["ccli", "page", "edit", "12345"]).expect("parse");
        match cli.command {
            Some(Commands::Page(PageArgs { command: PageCommands::Edit { page_id } })) => {
                assert_eq!(page_id, "12345");
            }
            other => panic!("expected Commands::Page(Edit), got {:?}", other),
        }
    }

    #[test]
    fn parses_blog_list_with_space_key() {
        let cli = Cli::try_parse_from(["ccli", "blog", "list", "DEV"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Blog(BlogArgs { command: BlogCommands::List { .. } }))
        ));
    }

    #[test]
    fn parses_blog_view_with_post_id() {
        let cli = Cli::try_parse_from(["ccli", "blog", "view", "77"]).expect("parse");
        match cli.command {
            Some(Commands::Blog(BlogArgs { command: BlogCommands::View { post_id } })) => {
                assert_eq!(post_id, "77");
            }
            other => panic!("expected Commands::Blog(View), got {:?}", other),
        }
    }

    #[test]
    fn parses_blog_create_without_parent_flag() {
        let cli = Cli::try_parse_from([
            "ccli", "blog", "create", "--space", "DEV", "--title", "Hi",
        ]).expect("parse");
        match cli.command {
            Some(Commands::Blog(BlogArgs { command: BlogCommands::Create { space, title } })) => {
                assert_eq!(space.as_deref(), Some("DEV"));
                assert_eq!(title.as_deref(), Some("Hi"));
            }
            other => panic!("expected Commands::Blog(Create), got {:?}", other),
        }
    }

    #[test]
    fn blog_create_with_parent_flag_fails_pitfall5() {
        // Pitfall 5 + D-37: blog posts have no parent. The --parent flag must NOT exist on BlogCommands::Create.
        let result = Cli::try_parse_from([
            "ccli", "blog", "create", "--space", "DEV", "--title", "Hi", "--parent", "99",
        ]);
        assert!(result.is_err(), "Pitfall 5: --parent must not be valid on `ccli blog create`");
    }

    #[test]
    fn parses_blog_edit_with_post_id() {
        let cli = Cli::try_parse_from(["ccli", "blog", "edit", "77"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Blog(BlogArgs { command: BlogCommands::Edit { .. } }))
        ));
    }
}
