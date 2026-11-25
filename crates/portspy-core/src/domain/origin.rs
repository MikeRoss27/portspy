//! Origin inference: *what caused this process to exist*.
//!
//! The origin is the interesting process in the ancestor chain — the AI
//! coding agent, IDE or terminal that started the dev server.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::confidence::Confidence;
use super::process::ProcessId;

/// Recognized origins in `v0.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OriginKind {
    Codex,
    ClaudeCode,
    Cursor,
    Zed,
    VsCode,
    WindowsTerminal,
    PowerShell,
    Cmd,
    Wsl,
    Docker,
    Unknown,
}

impl OriginKind {
    /// Stable identifier used in JSON output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
            Self::Zed => "zed",
            Self::VsCode => "vscode",
            Self::WindowsTerminal => "windows-terminal",
            Self::PowerShell => "powershell",
            Self::Cmd => "cmd",
            Self::Wsl => "wsl",
            Self::Docker => "docker",
            Self::Unknown => "unknown",
        }
    }

    /// Human-readable label for display.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::Zed => "Zed",
            Self::VsCode => "VS Code",
            Self::WindowsTerminal => "Windows Terminal",
            Self::PowerShell => "PowerShell",
            Self::Cmd => "CMD",
            Self::Wsl => "WSL2",
            Self::Docker => "Docker",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for OriginKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Inferred origin of a listener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginInfo {
    pub kind: OriginKind,
    pub label: String,
    pub confidence: Confidence,
    /// Process in the ancestor chain that matched the origin rule, when one
    /// could be identified.
    pub matched_pid: Option<ProcessId>,
}

impl OriginInfo {
    /// Creates a new inference.
    pub const fn new(
        kind: OriginKind,
        label: String,
        confidence: Confidence,
        matched_pid: Option<ProcessId>,
    ) -> Self {
        Self {
            kind,
            label,
            confidence,
            matched_pid,
        }
    }
}

impl Default for OriginInfo {
    fn default() -> Self {
        Self {
            kind: OriginKind::Unknown,
            label: OriginKind::Unknown.label().to_owned(),
            confidence: Confidence::Unknown,
            matched_pid: None,
        }
    }
}
