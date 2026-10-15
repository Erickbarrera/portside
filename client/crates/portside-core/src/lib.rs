//! Domain types shared by every Portside crate. No Tauri, no I/O beyond path lookup.

pub mod error;
pub mod paths;
pub mod platform;
pub mod types;

pub use error::CoreError;
pub use platform::Platform;
pub use types::{Arch, Os, PlayableStatus, PortType, StatusSource};
