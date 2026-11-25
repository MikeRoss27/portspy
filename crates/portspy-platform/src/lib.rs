//! PortSpy operating-system integration.
//!
//! The Windows implementation is isolated in this crate. `unsafe` is
//! permitted here and nowhere else in the workspace; every unsafe block
//! documents its invariants in a `// Safety:` comment.
//!
//! This crate depends on `portspy-core` for the [`Platform`] contract and
//! the domain types it fills with native data.

#[cfg(windows)]
pub mod windows;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod fallback;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;

#[cfg(windows)]
pub use windows::WindowsPlatform;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub use fallback::UnsupportedPlatform;

use portspy_core::platform::Platform;

/// Creates the platform implementation for the current operating system.
///
/// Windows, Linux and macOS use native backends.
pub fn platform() -> Box<dyn Platform + Sync> {
    #[cfg(windows)]
    {
        Box::new(WindowsPlatform)
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        Box::new(unix::UnixPlatform)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Box::new(UnsupportedPlatform)
    }
}
