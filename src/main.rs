use tracing_subscriber::EnvFilter;

mod api;

use api::error::AppError;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn"))
        )
        .init();

    if let Err(err) = run().await {
        let app_err = err
            .chain()
            .find_map(|e| e.downcast_ref::<AppError>())
            .cloned()
            .unwrap_or_else(|| AppError::Api(err.to_string()));
        handle_error(&app_err);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    Ok(())
}

// TODO: implement hint_for and handle_error in GREEN phase

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
        let wrapped: anyhow::Error =
            Err::<(), _>(inner).context("wrapper layer").unwrap_err();

        let recovered: Option<AppError> = wrapped
            .chain()
            .find_map(|e| e.downcast_ref::<AppError>())
            .cloned();

        match recovered {
            Some(AppError::Auth(msg)) => assert_eq!(msg, "bad token"),
            other => panic!("expected AppError::Auth recovered through chain, got {:?}", other),
        }
    }

    #[test]
    fn chain_walk_returns_none_when_no_app_error_present() {
        let plain: anyhow::Error = anyhow::anyhow!("just a string error");
        let recovered: Option<AppError> = plain
            .chain()
            .find_map(|e| e.downcast_ref::<AppError>())
            .cloned();
        assert!(recovered.is_none(), "plain anyhow error should not yield an AppError");
    }
}
