pub mod client;
pub mod error;

// Convenience re-exports for callers (Plan 05 init handler):
pub use client::{test_connection, Client};
pub use error::AppError;
