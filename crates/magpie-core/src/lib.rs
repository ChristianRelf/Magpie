//! Core domain types shared by every Magpie component.
//!
//! This crate intentionally contains no I/O. It defines the contracts between
//! the harness runtime, provider adapters, the routing engine, persistence and
//! the local API.

pub mod error;
pub mod execution;
pub mod limits;
pub mod model;
pub mod provider;
pub mod routing;
pub mod settings;
pub mod usage;

pub use error::*;
pub use execution::*;
pub use limits::*;
pub use model::*;
pub use provider::*;
pub use routing::*;
pub use settings::*;
pub use usage::*;

/// Product name used for binaries, paths and user agents.
pub const PRODUCT_NAME: &str = "Magpie";
/// Version of the harness, shared by the CLI, API and desktop app.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Local API protocol version. Bumped on breaking API changes so the desktop
/// app and CLI can detect an incompatible running harness.
pub const API_VERSION: u32 = 1;

pub fn new_id(prefix: &str) -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &id[..20])
}

pub type Timestamp = chrono::DateTime<chrono::Utc>;

pub fn now() -> Timestamp {
    chrono::Utc::now()
}
