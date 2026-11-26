//! Top-level error type of the PortSpy product logic.

use crate::platform::PlatformError;

/// Errors that can cross the product boundary.
#[derive(Debug, thiserror::Error)]
pub enum PortSpyError {
    /// An operating-system layer failure.
    #[error(transparent)]
    Platform(#[from] PlatformError),
    /// A requested listener does not exist in the latest snapshot.
    #[error("no listener on port {0}")]
    ListenerNotFound(u16),
    /// The process or listener scope changed after the stop was reviewed.
    #[error("stop canceled: the reviewed scope changed")]
    StopScopeChanged,
    /// Internal invariant violation. Reaching this is a bug.
    #[error("internal error: {0}")]
    Internal(String),
}
