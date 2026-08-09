//! Memory size formatting.

/// Formats a byte count for terminal display.
///
/// ```text
/// 612 MB
/// 1.4 GB
/// 96 KB
/// 512 B
/// ```
pub fn memory(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return "—".to_owned();
    };
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::memory;

    #[test]
    fn formats_bytes() {
        assert_eq!(memory(Some(512)), "512 B");
        assert_eq!(memory(Some(96 * 1024)), "96 KB");
        assert_eq!(memory(Some(612 * 1024 * 1024)), "612 MB");
        assert_eq!(memory(Some(1500 * 1024 * 1024)), "1.5 GB");
    }

    #[test]
    fn unknown_memory_is_a_dash() {
        assert_eq!(memory(None), "—");
    }
}
