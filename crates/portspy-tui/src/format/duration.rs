//! Uptime formatting.

use std::time::{Duration, SystemTime};

/// Formats an uptime for terminal display.
///
/// ```text
/// 12m    8m     1h     3d 2h    1m
/// ```
pub fn uptime(started_at: Option<SystemTime>, now: SystemTime) -> String {
    let Some(started) = started_at else {
        return "—".to_owned();
    };
    let elapsed = now.duration_since(started).unwrap_or(Duration::ZERO);
    duration(elapsed)
}

/// Formats a [`Duration`] compactly.
pub fn duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = hours / 24;
    format!("{}d {}h", days, hours % 24)
}

#[cfg(test)]
mod tests {
    use super::duration;
    use std::time::Duration;

    #[test]
    fn formats_seconds() {
        assert_eq!(duration(Duration::from_secs(5)), "5s");
    }

    #[test]
    fn formats_minutes() {
        assert_eq!(duration(Duration::from_secs(12 * 60)), "12m");
        assert_eq!(duration(Duration::from_secs(59 * 60 + 30)), "59m");
    }

    #[test]
    fn formats_hours() {
        assert_eq!(duration(Duration::from_secs(60 * 60)), "1h");
        assert_eq!(duration(Duration::from_secs(4 * 60 * 60)), "4h");
    }

    #[test]
    fn formats_days() {
        assert_eq!(
            duration(Duration::from_secs(3 * 24 * 60 * 60 + 2 * 3600)),
            "3d 2h"
        );
    }
}
