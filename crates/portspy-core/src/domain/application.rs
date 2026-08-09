//! Application inference: what the listening process *appears to be*.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::confidence::Confidence;

/// The application PortSpy believes a listener belongs to.
///
/// This is always an inference. A `node.exe` process may be Next.js, Vite or
/// anything else; the [`ApplicationInfo`] carries the best guess together
/// with its confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationKind {
    NextJs,
    Vite,
    Node,
    Python,
    Rust,
    PostgreSql,
    Redis,
    Docker,
    Unknown,
}

impl ApplicationKind {
    /// Stable identifier used in JSON output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NextJs => "nextjs",
            Self::Vite => "vite",
            Self::Node => "node",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::PostgreSql => "postgresql",
            Self::Redis => "redis",
            Self::Docker => "docker",
            Self::Unknown => "unknown",
        }
    }

    /// Human-readable label for display.
    pub const fn label(self) -> &'static str {
        match self {
            Self::NextJs => "Next.js",
            Self::Vite => "Vite",
            Self::Node => "Node.js",
            Self::Python => "Python",
            Self::Rust => "Rust",
            Self::PostgreSql => "PostgreSQL",
            Self::Redis => "Redis",
            Self::Docker => "Docker",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for ApplicationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Inferred application identity of a listener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationInfo {
    pub kind: ApplicationKind,
    pub label: String,
    pub confidence: Confidence,
}

impl ApplicationInfo {
    /// Creates a new inference.
    pub const fn new(kind: ApplicationKind, label: String, confidence: Confidence) -> Self {
        Self { kind, label, confidence }
    }
}

impl Default for ApplicationInfo {
    fn default() -> Self {
        Self {
            kind: ApplicationKind::Unknown,
            label: ApplicationKind::Unknown.label().to_owned(),
            confidence: Confidence::Unknown,
        }
    }
}
