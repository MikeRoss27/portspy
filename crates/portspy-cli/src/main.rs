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
use std::path::PathBuf;

use args::{Cli, Command};

fn main() -> Result<(), anyhow::Error> {
    init_tracing()?;

    let cli = Cli::parse();
    let platform = portspy_platform::platform();
    let mut builder = SnapshotBuilder::new(platform.as_ref());

    let command = cli.command.unwrap_or_default();
    match command {
        Command::Tui => {
            let platform = std::sync::Arc::from(platform);
            let app = portspy_tui::app::App::new(platform);
            app.run()?;
        }
        Command::List { json } => commands::list(&mut builder, json)?,
        Command::Inspect {
            port,
            protocol,
            json,
        } => commands::inspect(&mut builder, port, protocol.map(Into::into), json)?,
        Command::Kill {
            port,
            protocol,
            tree,
            yes,
        } => commands::kill(
            &mut builder,
            platform.as_ref(),
            port,
            protocol.map(Into::into),
            tree,
            yes,
        )?,
    }
    Ok(())
}

/// Enables `tracing` when `PORTSPY_LOG` is set.
/// Logs go to stderr and to a rotating file in %LOCALAPPDATA%\PortSpy\logs\.
/// This keeps the terminal interface and JSON output clean on stdout.
fn init_tracing() -> Result<(), anyhow::Error> {
    use tracing_subscriber::{
        EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    let filter = EnvFilter::try_from_env("PORTSPY_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));

    // Console layer (stderr) - for immediate feedback
    let console_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_filter(filter.clone());

    // File layer - for persistent debugging
    let file_layer = {
        let log_dir = log_directory()?;
        std::fs::create_dir_all(&log_dir)?;
        let date = time::OffsetDateTime::now_local()
            .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
            .format(time::macros::format_description!("[year][month][day]"))?;
        let file_path = log_dir.join(format!("portspy-{date}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;
        fmt::layer()
            .with_writer(file)
            .with_ansi(false)
            .with_filter(filter)
    };

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .try_init()?;

    Ok(())
}

/// Returns the log directory: %LOCALAPPDATA%\PortSpy\logs\ on Windows,
/// ~/.local/share/portspy/logs on Unix.
fn log_directory() -> Result<PathBuf, anyhow::Error> {
    #[cfg(windows)]
    {
        use std::env;
        let local_app_data = env::var("LOCALAPPDATA").map(PathBuf::from).or_else(|_| {
            env::var("USERPROFILE").map(|p| PathBuf::from(p).join("AppData").join("Local"))
        })?;
        Ok(local_app_data.join("PortSpy").join("logs"))
    }
    #[cfg(not(windows))]
    {
        use std::env;
        let data_dir = env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| env::var("HOME").map(|p| PathBuf::from(p).join(".local").join("share")))?;
        Ok(data_dir.join("portspy").join("logs"))
    }
}
