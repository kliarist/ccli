pub mod client;
pub mod error;
pub mod page;
pub mod space;

// Convenience re-exports for callers:
pub use client::{test_connection, Client};
pub use error::AppError;
pub use page::{ContentType, Page, PageDetail};
pub use space::{get_space_detail, list_all_spaces, resolve_webui_url, Space, SpaceDetail};
