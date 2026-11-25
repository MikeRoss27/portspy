//! Origin detection: which process in the ancestor chain caused this one?
//!
//! # Priority
//!
//! Origins are matched in this order:
//!
//! ```text
//! AI coding agent  →  IDE  →  terminal  →  environment  →  unknown
//! ```
//!
//! # Semantics
//!
//! The origin is the *most interesting* process in the ancestor chain, not
//! the terminal the user happened to open. A chain like
//!
//! ```text
//! WindowsTerminal.exe → pwsh.exe → codex.exe → node.exe
//! ```
//!
//! must resolve to `Codex` — not to `Windows Terminal`.
//!
//! # Behavior contract for the implementation
//!
//! * Each rule matches a process by executable name, and optionally by
//!   command-line content.
//! * Rules are evaluated from most specific to least specific; the first
//!   match wins, so `Codex` beats `PowerShell` beats `Windows Terminal`.
//! * `matched_pid` records the process that matched.
//! * Confidence: exact executable match on the process itself is `Exact`;
//!   a match in the ancestor chain is `High`; a weak substring match is
//!   `Low`. When nothing matches, `Unknown` with `Confidence::Unknown`.
//! * Rules live in [`rules`] as declarative data.

pub mod rules;

use crate::detection::DetectionContext;
use crate::domain::origin::OriginInfo;
use crate::domain::process::ProcessInfo;
use crate::processes::ancestry::ProcessAncestry;

/// Rule-based origin detector.
///
/// # Errors
///
/// This detector never fails: missing or inaccessible data yields
/// [`OriginKind::Unknown`].
#[derive(Debug, Clone, Default)]
pub struct OriginDetector;

impl OriginDetector {
    /// Creates the default detector with the built-in rule set.
    pub fn new() -> Self {
        Self
    }

    /// Detects the origin of `process` given its ancestor chain.
    pub fn detect(
        &self,
        context: &DetectionContext<'_>,
        process: &ProcessInfo,
        ancestry: &ProcessAncestry,
    ) -> OriginInfo {
        crate::detection::origin::rules::detect_origin(context, process, ancestry)
    }
}
