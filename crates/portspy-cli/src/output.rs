//! Machine-readable output contract.
//!
//! The JSON interface and the TUI share the same core domain model; this
//! module defines the stable serialization shape so the CLI contract does
//! not drift.

use portspy_core::domain::application::ApplicationKind;
use portspy_core::domain::confidence::Confidence;
use portspy_core::domain::origin::OriginKind;
use serde::Serialize;

/// One listener as exposed over JSON.
#[derive(Debug, Serialize)]
pub struct ListenerJson {
    pub port: u16,
    pub protocol: &'static str,
    pub address: std::net::IpAddr,
    pub application: ApplicationJson,
    pub process: ProcessJson,
    pub origin: OriginJson,
    pub project: ProjectJson,
    pub is_duplicate: bool,
    pub is_orphan: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectJson {
    pub name: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Serialize)]
pub struct ApplicationJson {
    pub kind: ApplicationKind,
    pub confidence: Confidence,
}

#[derive(Debug, Serialize)]
pub struct ProcessJson {
    pub pid: u32,
    pub name: String,
    pub memory_bytes: Option<u64>,
    pub started_at: Option<std::time::SystemTime>,
}

#[derive(Debug, Serialize)]
pub struct OriginJson {
    pub kind: OriginKind,
    pub confidence: Confidence,
}

impl From<&portspy_core::domain::listener::Listener> for ListenerJson {
    fn from(listener: &portspy_core::domain::listener::Listener) -> Self {
        Self {
            port: listener.port,
            protocol: listener.protocol.as_str(),
            address: listener.address,
            application: ApplicationJson {
                kind: listener.application.kind,
                confidence: listener.application.confidence,
            },
            process: ProcessJson {
                pid: listener.pid().as_u32(),
                name: listener.process.name.clone(),
                memory_bytes: listener.process.memory_bytes,
                started_at: listener.process.started_at,
            },
            origin: OriginJson {
                kind: listener.origin.kind,
                confidence: listener.origin.confidence,
            },
            project: ProjectJson {
                name: listener.project.name().map(str::to_owned),
                confidence: listener.project.confidence,
            },
            is_duplicate: listener.is_duplicate,
            is_orphan: listener.is_orphan,
        }
    }
}

/// The JSON documents this CLI emits.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Output {
    List { listeners: Vec<ListenerJson> },
    Inspect { listener: Option<ListenerJson> },
}

#[cfg(test)]
mod tests {
    use super::Output;
    use serde_json::json;

    #[test]
    fn list_output_has_no_internal_enum_wrapper() {
        let value = serde_json::to_value(Output::List {
            listeners: Vec::new(),
        });

        assert_eq!(value.ok(), Some(json!({ "listeners": [] })));
    }

    #[test]
    fn inspect_output_has_no_internal_enum_wrapper() {
        let value = serde_json::to_value(Output::Inspect { listener: None });

        assert_eq!(value.ok(), Some(json!({ "listener": null })));
    }
}
