//! The OS and architecture this binary runs on.

use crate::types::{Arch, Os};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// The platform this binary was compiled for. Portside only ships for the OSes in [`Os`].
    pub const fn current() -> Platform {
        let os = if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else if cfg!(target_os = "android") {
            Os::Android
        } else {
            Os::Linux
        };
        let arch = if cfg!(target_arch = "aarch64") {
            Arch::Arm64
        } else {
            Arch::X86_64
        };
        Platform { os, arch }
    }
}
