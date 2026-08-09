//! PortSpy executable: composition point.
//!
//! ```text
//! platform
//!    ↓
//! core
//!    ↓
//! TUI / CLI output
//! ```

mod args;
mod commands;
mod output;

use clap::Parser;
use portspy_core::snapshot::builder::SnapshotBuilder;

use args::{Cli, Command};

fn main() -> Result<(), anyhow::Error> {
    init_tracing();

    let cli = Cli::parse();
    let platform = portspy_platform::platform();
    let builder = SnapshotBuilder::new(platform.as_ref());

    let command = cli.command.unwrap_or_default();
    match command {
        Command::Tui => {
            let platform = std::sync::Arc::from(platform);
            let app = portspy_tui::app::App::new(platform);
            app.run()?;
        }
        Command::List { json } => commands::list(&builder, json)?,
        Command::Inspect { port, json } => commands::inspect(&builder, port, json)?,
        Command::Kill { port, yes } => commands::kill(&builder, platform.as_ref(), port, yes)?,
    }
    Ok(())
}

/// Enables `tracing` when `PORTSPY_LOG` is set. Logs go to stderr so the
/// terminal interface and JSON output stay clean on stdout.
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_env("PORTSPY_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
