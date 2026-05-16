use is_terminal::IsTerminal;
use tracing_subscriber::EnvFilter;

mod api;
mod cli;
mod config;
mod output;
mod tui;

use api::error::AppError;
use cli::{Cli, Commands, SpaceCommands};

#[tokio::main]
async fn main() {
    // RUST_LOG-driven tracing init (D-05). Default to `warn` if RUST_LOG unset.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    if let Err(err) = run().await {
        // Walk the full anyhow chain so any `.context("...")` wrapper does not
        // defeat AppError category dispatch (D-04). If no AppError is in the
        // chain, fall back to AppError::Api with the top-level message.
        let app_err = err
            .chain()
            .find_map(|e| e.downcast_ref::<AppError>())
            .cloned()
            .unwrap_or_else(|| AppError::Api(err.to_string()));
        handle_error(&app_err);
        std::process::exit(1);
    }
}

fn cli_styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Effects, Styles};
    Styles::styled()
        .header(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Red.on_default() | Effects::BOLD)
}

async fn run() -> anyhow::Result<()> {
    use clap::{CommandFactory, FromArgMatches};
    let matches = Cli::command().styles(cli_styles()).get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    match cli.command {
        Some(Commands::Init) => cli::init::run(&cli).await,
        Some(Commands::Space(ref args)) => match args.command {
            SpaceCommands::List => cli::space::run(&cli, args).await,
        },
        Some(Commands::Page(ref args)) => cli::page::run(&cli, args).await,
        Some(Commands::Blog(ref args)) => cli::blog::run(&cli, args).await,
        Some(Commands::Comment(ref args)) => cli::comment::run(&cli, args).await,
        Some(Commands::Attachment(ref args)) => cli::attachment::run(&cli, args).await,
        None => tui::run().await, // D-10: bare ccli launches TUI
    }
}

/// Pure mapping from AppError variant -> remediation hint string.
///
/// Exposed (crate-private) so unit tests can verify category dispatch
/// without spinning up stderr capture. This is the heart of D-04.
fn hint_for(err: &AppError) -> &'static str {
    match err {
        AppError::Auth(_) => "Run 'ccli init' to reconfigure your credentials.",
        AppError::Network(_) => "Check that your Confluence instance is reachable.",
        AppError::Config(_) => "Run 'ccli init' to set up your configuration.",
        AppError::Api(_) => "Check your Confluence instance status.",
    }
}

/// Two-line stderr error format per D-04:
///   line 1: `Error: <message>`  (red bold prefix when stderr is a terminal)
///   line 2: remediation hint   (yellow when stderr is a terminal)
fn handle_error(err: &AppError) {
    if std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        eprintln!("\x1b[1;31mError:\x1b[0m {}", err);
        eprintln!("\x1b[33m{}\x1b[0m", hint_for(err));
    } else {
        eprintln!("Error: {}", err);
        eprintln!("{}", hint_for(err));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_for_auth_returns_init_credentials_message() {
        assert_eq!(
            hint_for(&AppError::Auth("x".into())),
            "Run 'ccli init' to reconfigure your credentials."
        );
    }

    #[test]
    fn hint_for_network_returns_reachable_message() {
        assert_eq!(
            hint_for(&AppError::Network("x".into())),
            "Check that your Confluence instance is reachable."
        );
    }

    #[test]
    fn hint_for_config_returns_init_setup_message() {
        assert_eq!(
            hint_for(&AppError::Config("x".into())),
            "Run 'ccli init' to set up your configuration."
        );
    }

    #[test]
    fn hint_for_api_returns_status_message() {
        assert_eq!(
            hint_for(&AppError::Api("x".into())),
            "Check your Confluence instance status."
        );
    }

    /// Closes the loop on B-02: the chain walk used in main() must recover
    /// the inner AppError even when anyhow::Context has wrapped it. If this
    /// test fails, every `.context("...")` call in downstream plans would
    /// silently demote category dispatch to the Api fallback hint.
    #[test]
    fn chain_walk_recovers_inner_app_error_through_context_wrapper() {
        use anyhow::Context;
        let inner = AppError::Auth("bad token".into());
        let wrapped: anyhow::Error = Err::<(), _>(inner).context("wrapper layer").unwrap_err();

        let recovered: Option<AppError> = wrapped
            .chain()
            .find_map(|e| e.downcast_ref::<AppError>())
            .cloned();

        match recovered {
            Some(AppError::Auth(msg)) => assert_eq!(msg, "bad token"),
            other => panic!(
                "expected AppError::Auth recovered through chain, got {:?}",
                other
            ),
        }
    }

    /// Sanity check: when there is NO AppError anywhere in the chain, the
    /// fallback to AppError::Api(err.to_string()) is what main() will use.
    #[test]
    fn chain_walk_returns_none_when_no_app_error_present() {
        let plain: anyhow::Error = anyhow::anyhow!("just a string error");
        let recovered: Option<AppError> = plain
            .chain()
            .find_map(|e| e.downcast_ref::<AppError>())
            .cloned();
        assert!(
            recovered.is_none(),
            "plain anyhow error should not yield an AppError"
        );
    }
}
