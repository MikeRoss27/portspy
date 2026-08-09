//! Application detection: what application is this process?
//!
//! # Priority
//!
//! Detection is attempted from most specific to least specific:
//!
//! ```text
//! specific framework  →  runtime  →  raw executable  →  unknown
//! ```
//!
//! Examples:
//!
//! ```text
//! node.exe + command line contains "next dev"   → Next.js (High)
//! node.exe + command line contains "vite"       → Vite (High)
//! node.exe                                       → Node.js (Exact)
//! python.exe                                     → Python (Exact)
//! cargo run / target\*.exe                       → Rust (Medium)
//! postgres.exe                                   → PostgreSQL (Exact)
//! ```
//!
//! PortSpy prefers precise information when it is proven and falls back
//! cleanly to more general information otherwise.

pub mod rules;

use crate::domain::application::ApplicationInfo;
use crate::domain::process::ProcessInfo;

/// Rule-based application detector.
///
/// # Errors
///
/// This detector never fails: an unrecognized process yields
/// [`ApplicationKind::Unknown`].
#[derive(Debug, Clone, Default)]
pub struct ApplicationDetector;

impl ApplicationDetector {
    /// Creates the default detector with the built-in rule set.
    pub fn new() -> Self {
        Self
    }

    /// Detects the application of a process.
    pub fn detect(&self, process: &ProcessInfo) -> ApplicationInfo {
        let (kind, confidence) = crate::detection::application::rules::detect_application(process);
        ApplicationInfo::new(kind, kind.label().to_owned(), confidence)
    }
}
