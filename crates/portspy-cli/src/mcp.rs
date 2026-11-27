//! Read-only MCP tools over the standard I/O transport.

use std::net::IpAddr;
use std::sync::Arc;

use portspy_core::domain::listener::Listener;
use portspy_core::domain::process::ProcessAncestor;
use portspy_core::domain::protocol::Protocol;
use portspy_core::platform::Platform;
use portspy_core::ports::Exposure;
use portspy_core::snapshot::{SystemSnapshot, builder::SnapshotBuilder};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::output::ListenerJson;

pub(crate) fn run(platform: Arc<dyn Platform + Sync>) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            PortSpy { platform }
                .serve(rmcp::transport::stdio_pipes())
                .await?
                .waiting()
                .await?;
            Ok(())
        })
}

#[derive(Clone)]
struct PortSpy {
    platform: Arc<dyn Platform + Sync>,
}

#[derive(Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Transport {
    Tcp,
    Udp,
}

impl From<Transport> for Protocol {
    fn from(value: Transport) -> Self {
        match value {
            Transport::Tcp => Self::Tcp,
            Transport::Udp => Self::Udp,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct InspectArgs {
    /// Numeric local port number.
    port: u16,
    /// Set when you only want TCP or UDP.
    protocol: Option<Transport>,
    /// Exact local bind address, without brackets or a port.
    address: Option<IpAddr>,
}

#[derive(Serialize)]
struct Observation<'a> {
    #[serde(flatten)]
    listener: ListenerJson,
    exposure: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ancestors: Option<&'a [ProcessAncestor]>,
}

impl<'a> Observation<'a> {
    fn new(listener: &'a Listener, inspect: bool) -> Self {
        let exposure = match Exposure::of(listener.address) {
            Exposure::Loopback => "loopback",
            Exposure::Interface => "interface",
            Exposure::AllInterfaces => "all_interfaces",
        };
        Self {
            listener: listener.into(),
            exposure,
            ancestors: inspect.then_some(listener.process.ancestors.as_slice()),
        }
    }
}

#[tool_router]
impl PortSpy {
    #[tool(
        description = "List local TCP listeners and bound UDP sockets with their bind addresses, visible owners, service attribution, and network exposure.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn list_listeners(&self) -> Result<CallToolResult, ErrorData> {
        let snapshot = self.scan().await?;
        let listeners: Vec<_> = snapshot
            .listeners
            .iter()
            .map(|listener| Observation::new(listener, false))
            .collect();
        Ok(CallToolResult::structured(
            json!({ "listeners": listeners }),
        ))
    }

    #[tool(
        description = "Inspect all observed sockets on a local port, with optional protocol and bind address. Returns process ancestors when available.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn inspect_listener(
        &self,
        Parameters(args): Parameters<InspectArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let snapshot = self.scan().await?;
        let protocol = args.protocol.map(Protocol::from);
        let listeners: Vec<_> = snapshot
            .listeners
            .iter()
            .filter(|listener| {
                listener.port == args.port
                    && protocol.is_none_or(|protocol| listener.protocol == protocol)
                    && args
                        .address
                        .is_none_or(|address| listener.address == address)
            })
            .map(|listener| Observation::new(listener, true))
            .collect();
        Ok(CallToolResult::structured(
            json!({ "listeners": listeners }),
        ))
    }

    async fn scan(&self) -> Result<SystemSnapshot, ErrorData> {
        let platform = Arc::clone(&self.platform);
        tokio::task::spawn_blocking(move || SnapshotBuilder::new(platform.as_ref()).snapshot())
            .await
            .map_err(|_| ErrorData::internal_error("Listener scan task failed", None))?
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))
    }
}

#[tool_handler]
impl ServerHandler for PortSpy {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("portspy", env!("CARGO_PKG_VERSION")))
            .with_instructions("Inspect local listening sockets. A missing PID or an empty result can reflect limited visibility.")
    }
}
