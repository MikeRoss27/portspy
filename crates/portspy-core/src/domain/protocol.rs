//! Transport protocol of a listener.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol carried by a listener.
///
/// `v0.1` only discovers TCP listeners. UDP arrives later, so the enum is
/// designed to grow without breaking existing consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
}

impl Protocol {
    /// Stable lowercase name, used in JSON output and display.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
