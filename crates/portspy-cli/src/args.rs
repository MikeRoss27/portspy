//! Command-line argument parsing.

use clap::{Parser, Subcommand};

/// PortSpy: know what's using your localhost.
#[derive(Debug, Parser)]
#[command(name = "portspy", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
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
        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Stops the process owning a port
    Kill {
        /// Local port number
        port: u16,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}
