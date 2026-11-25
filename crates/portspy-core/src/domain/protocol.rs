//! Transport protocol of a listener.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol carried by a listener.
///
/// The enum is designed to grow (QUIC, ...) without breaking existing
/// consumers: every consumer renders through [`Protocol::as_str`] and the
/// identity of a listener always includes the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Protocol {
    /// Stable lowercase name, used in JSON output and display.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
