//! Confidence level attached to every inference.

use serde::{Deserialize, Serialize};

/// How sure PortSpy is about an inferred piece of information.
///
/// Inferences carry confidence; facts do not. Every detection result carries
/// one of these so the UI and JSON output can be honest about certainty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// No evidence at all; this is a fallback value, not a detection.
    Unknown,
    /// Weak evidence (path substring, fuzzy match).
    Low,
    /// Plausible evidence (generic runtime with a known command line).
    Medium,
    /// Strong but not conclusive evidence (ancestor matches a known origin).
    High,
    /// Direct, unambiguous evidence (exact executable name match).
    Exact,
}
