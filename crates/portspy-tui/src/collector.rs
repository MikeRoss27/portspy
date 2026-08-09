//! Snapshot collector: refreshes the view on a fixed cadence.
//!
//! A dedicated thread rebuilds [`SystemSnapshot`]s so the UI thread never
//! blocks on OS enumeration. Snapshots travel over an mpsc channel and are
//! applied between frames.

use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use portspy_core::platform::Platform;
use portspy_core::snapshot::builder::SnapshotBuilder;

use crate::app::Message;

/// Interval between two snapshot refreshes.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// Spawns the collector thread.
///
/// The thread lives until the channel closes (i.e. until the app exits).
///
/// # Errors
///
/// Returns an error when the operating system refuses to create the thread.
pub fn spawn(
    platform: std::sync::Arc<dyn Platform + Sync>,
    tx: Sender<Message>,
) -> std::io::Result<thread::JoinHandle<()>> {
    thread::Builder::new()
        .name("portspy-collector".to_owned())
        .spawn(move || {
            let builder = SnapshotBuilder::new(platform.as_ref());
            loop {
                let snapshot = builder.snapshot();
                match snapshot {
                    Ok(snapshot) => {
                        if tx.send(Message::Snapshot(snapshot)).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        if tx.send(Message::CollectionError(error)).is_err() {
                            return;
                        }
                    }
                }
                thread::sleep(REFRESH_INTERVAL);
            }
        })
}

#[cfg(test)]
mod tests {
    use super::REFRESH_INTERVAL;

    #[test]
    fn refresh_interval_is_in_the_seconds_range() {
        assert!(REFRESH_INTERVAL >= std::time::Duration::from_secs(1));
        assert!(REFRESH_INTERVAL <= std::time::Duration::from_secs(5));
    }
}
