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
mod mcp;
mod output;

use clap::Parser;
use portspy_core::snapshot::builder::SnapshotBuilder;
use std::fs::File;
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
        Command::Mcp => mcp::run(std::sync::Arc::from(platform))?,
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

/// Sends logs to stderr and, when available, to a daily log file.
/// `PORTSPY_LOG` overrides the default filter.
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

    let (file, file_error) = match open_log_file() {
        Ok(file) => (Some(file), None),
        Err(error) => (None, Some(error)),
    };
    let file_layer = file.map(|file| {
        fmt::layer()
            .with_writer(file)
            .with_ansi(false)
            .with_filter(filter)
    });

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .try_init()?;

    if let Some(error) = file_error {
        tracing::warn!(%error, "Could not open log file");
    }

    Ok(())
}

fn open_log_file() -> Result<File, anyhow::Error> {
    let log_dir = log_directory()?;
    std::fs::create_dir_all(&log_dir)?;
    let date = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
        .format(time::macros::format_description!("[year][month][day]"))?;
    let file_path = log_dir.join(format!("portspy-{date}.log"));
    Ok(File::options().create(true).append(true).open(file_path)?)
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
