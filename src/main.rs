use is_terminal::IsTerminal;
use tracing_subscriber::EnvFilter;

mod api;
mod cache;
mod cli;
mod config;
mod edit;
mod output;
mod tui;

use api::error::AppError;
use cli::{Cli, Commands, SpaceCommands};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    if let Err(err) = run().await {
        // Walk the full anyhow chain; .context() wrappers must not hide the inner AppError.
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
        None => tui::run().await,
    }
}

/// Maps an AppError variant to a user-facing remediation hint.
fn hint_for(err: &AppError) -> &'static str {
    match err {
        AppError::Auth(_) => "Run 'ccli init' to reconfigure your credentials.",
        AppError::Network(_) => "Check that your Confluence instance is reachable.",
        AppError::Config(_) => "Run 'ccli init' to set up your configuration.",
        AppError::Api(_) => "Check your Confluence instance status.",
    }
}

/// Print a two-line error to stderr: message + remediation hint.
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

    #[test]
    fn handle_error_does_not_panic_for_any_variant() {
        // stderr is not a terminal in tests — exercises the plain-text branch
        handle_error(&AppError::Auth("auth error".into()));
        handle_error(&AppError::Network("network error".into()));
        handle_error(&AppError::Config("config error".into()));
        handle_error(&AppError::Api("api error".into()));
    }

    #[test]
    fn cli_styles_returns_without_panic() {
        let _ = cli_styles();
    }
}
