//! `ccli blog` subcommand handlers — delegates to page handlers with ContentType::BlogPost.

use crate::api::page::ContentType;
use crate::cli::page;
use crate::cli::{BlogArgs, BlogCommands, Cli};

/// Top-level dispatcher for `ccli blog` subcommands. Delegates to typed
/// helpers in `cli::page` with `ContentType::BlogPost`.
pub async fn run(cli: &Cli, args: &BlogArgs) -> anyhow::Result<()> {
    match &args.command {
        BlogCommands::List { space_key } => {
            page::handle_list_typed(cli, space_key, ContentType::BlogPost).await
        }
        BlogCommands::View { post_id } => page::handle_view_typed(post_id).await,
        BlogCommands::Create { space, title } => {
            page::handle_create_typed(
                space.as_deref(),
                title.as_deref(),
                None,
                ContentType::BlogPost,
            )
            .await
        }
        BlogCommands::Edit { post_id, raw } => {
            page::handle_edit_typed(post_id, ContentType::BlogPost, *raw).await
        }
    }
}
