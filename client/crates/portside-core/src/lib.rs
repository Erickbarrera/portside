//! Domain types shared by every Portside crate. No Tauri, no I/O beyond path lookup (and
//! making the HTTP client the other crates use).

pub mod error;
pub mod http;
pub mod paths;
pub mod platform;
pub mod types;

pub use error::CoreError;
pub use platform::Platform;
pub use types::{Arch, Os, PlayableStatus, PortType, StatusSource};
