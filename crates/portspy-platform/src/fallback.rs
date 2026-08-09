//! Stub platform used on non-Windows systems.
//!
//! PortSpy is Windows-first; on other platforms every operation reports
//! itself as unsupported instead of pretending to work.

use portspy_core::domain::process::ProcessId;
use portspy_core::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};

/// Placeholder implementation for non-Windows targets.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedPlatform;

impl Platform for UnsupportedPlatform {
    fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
        Err(PlatformError::Unsupported("listener discovery"))
    }

    fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
        Err(PlatformError::Unsupported("process inspection"))
    }

    fn kill(&self, _pid: ProcessId) -> Result<KillOutcome, PlatformError> {
        Err(PlatformError::Unsupported("process termination"))
    }
}
