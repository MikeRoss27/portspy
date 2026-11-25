//! Command-line argument parsing.

use clap::{Parser, Subcommand, ValueEnum};

/// PortSpy: know what's using your localhost.
#[derive(Debug, Parser)]
#[command(name = "portspy", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Transport protocol selector for port-targeted commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProtocolArg {
    Tcp,
    Udp,
}

impl From<ProtocolArg> for portspy_core::domain::protocol::Protocol {
    fn from(value: ProtocolArg) -> Self {
        match value {
            ProtocolArg::Tcp => Self::Tcp,
            ProtocolArg::Udp => Self::Udp,
        }
    }
}

#[derive(Debug, Subcommand, Default)]
pub enum Command {
    /// Opens the interactive terminal interface (default)
    #[default]
    Tui,
    /// Lists local listening ports
    List {
        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Shows details about the process behind a port
    Inspect {
        /// Local port number
        port: u16,
        /// Transport protocol when the port is shared by TCP and UDP
        #[arg(long, value_enum)]
        protocol: Option<ProtocolArg>,
        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Stops the process owning a port
    Kill {
        /// Local port number
        port: u16,
        /// Transport protocol when the port is shared by TCP and UDP
        #[arg(long, value_enum)]
        protocol: Option<ProtocolArg>,
        /// Also stop every descendant process (process tree)
        #[arg(long)]
        tree: bool,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}
