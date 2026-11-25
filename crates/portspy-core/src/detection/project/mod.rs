//! Project detection: which source project does a process belong to?
//!
//! # Semantics
//!
//! The project name is derived from the process working directory: a process
//! whose cwd is `D:\dev\bengal` belongs to the `bengal` project. Working
//! directories that point at system folders or user homes yield `Unknown`.
//!
//! # Behavior contract
//!
//! * The project name is the last path segment of the working directory.
//! * Known non-project folders (`node_modules`, `System32`, a user home, ...)
//!   are rejected with `Unknown`.
//! * A missing working directory is `Unknown`, never a guess.
//! * Rules live in [`rules`] as declarative data.

pub mod rules;

use crate::domain::process::ProcessInfo;
use crate::domain::project::ProjectInfo;

/// Rule-based project detector.
///
/// # Errors
///
/// This detector never fails: a process without a usable working directory
/// yields [`crate::domain::project::ProjectKind::Unknown`].
#[derive(Debug, Clone, Default)]
pub struct ProjectDetector;

impl ProjectDetector {
    /// Creates the default detector with the built-in rule set.
    pub fn new() -> Self {
        Self
    }

    /// Detects the project of a process.
    pub fn detect(&self, process: &ProcessInfo) -> ProjectInfo {
        let (kind, confidence) = crate::detection::project::rules::detect_project(process);
        ProjectInfo::new(kind, confidence)
    }
}
