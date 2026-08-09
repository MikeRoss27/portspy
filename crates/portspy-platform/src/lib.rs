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

#[cfg(not(windows))]
mod fallback;

#[cfg(windows)]
pub use windows::WindowsPlatform;

#[cfg(not(windows))]
pub use fallback::UnsupportedPlatform;

use portspy_core::platform::Platform;

/// Creates the platform implementation for the current operating system.
///
/// Windows gets the real implementation; every other OS gets a stub that
/// reports every operation as unsupported, so the workspace still builds
/// and runs (its tests) outside Windows.
pub fn platform() -> Box<dyn Platform + Sync> {
    #[cfg(windows)]
    {
        Box::new(WindowsPlatform)
    }
    #[cfg(not(windows))]
    {
        Box::new(UnsupportedPlatform)
    }
}
