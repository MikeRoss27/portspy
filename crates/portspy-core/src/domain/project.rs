//! Project inference: which source project a listener belongs to.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::confidence::Confidence;

/// The project PortSpy believes a listener's process works on.
///
/// This is always an inference. A working directory is a strong hint but the
/// folder name alone cannot prove that the directory is a real project, so
/// the [`ProjectInfo`] carries a confidence like every other inference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectKind {
    /// A working directory was mapped to a project name.
    Named(String),
    /// No usable project could be derived from the working directory.
    Unknown,
}

impl ProjectKind {
    /// The project name, when one is known.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named(name) => Some(name),
            Self::Unknown => None,
        }
    }

    /// Human-readable label for display.
    pub fn label(&self) -> &str {
        self.name().unwrap_or("Unknown")
    }
}

impl fmt::Display for ProjectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Inferred project identity of a listener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub kind: ProjectKind,
    pub confidence: Confidence,
}

impl ProjectInfo {
    /// Creates a new inference.
    pub const fn new(kind: ProjectKind, confidence: Confidence) -> Self {
        Self { kind, confidence }
    }

    /// Human-readable label for display.
    pub fn label(&self) -> &str {
        self.kind.label()
    }

    /// The project name, when one is known.
    pub fn name(&self) -> Option<&str> {
        self.kind.name()
    }
}

impl Default for ProjectInfo {
    fn default() -> Self {
        Self {
            kind: ProjectKind::Unknown,
            confidence: Confidence::Unknown,
        }
    }
}
