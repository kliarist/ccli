pub mod attachment;
pub mod client;
pub mod comment;
pub mod error;
pub mod page;
pub mod space;

// Convenience re-exports for callers:
pub use client::{is_cloud_url, normalize_url, test_connection, Client};
pub use error::{map_network_error, AppError};
#[allow(unused_imports)]
pub use page::{
    create_page, get_page_detail, list_all_pages, update_page, ContentType, Page, PageDetail,
};
pub use space::{get_space_detail, list_all_spaces, resolve_webui_url, Space, SpaceDetail};
