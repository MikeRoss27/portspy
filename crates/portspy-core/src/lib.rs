//! PortSpy core: the product logic.
//!
//! This crate contains no terminal code and no Windows API usage. It owns:
//!
//! * domain models (facts and inference)
//! * listener discovery orchestration through the [`platform::Platform`] contract
//! * process ancestry reconstruction
//! * application and origin detection
//! * snapshot construction and diffing
//!
//! Everything here is pure, testable product logic.

pub mod detection;
pub mod domain;
pub mod error;
pub mod platform;
pub mod ports;
pub mod processes;
pub mod snapshot;

pub use error::PortSpyError;
pub use platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
