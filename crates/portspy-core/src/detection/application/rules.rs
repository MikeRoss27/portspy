//! Application detection rules.
//!
//! Detection follows the priority:
//!
//! ```text
//! specific framework  →  runtime  →  raw executable  →  unknown
//! ```

use crate::domain::application::ApplicationKind;
use crate::domain::confidence::Confidence;
use crate::domain::process::ProcessInfo;

/// Detects the application of a process.
///
/// Rules are ordered from most specific to least specific; the first match
/// wins. Command-line rules are evaluated before the runtime fallback so a
/// `node.exe` running `next dev` is reported as Next.js rather than Node.js.
pub(crate) fn detect_application(process: &ProcessInfo) -> (ApplicationKind, Confidence) {
    let name = process.name.to_lowercase();
    let command_line = process.command_line.as_deref().unwrap_or("").to_lowercase();
    let tokens: Vec<&str> = command_line.split_whitespace().collect();

    if is_node(&name) {
        // Real Windows command lines contain paths with spaces, so the
        // `next` bin often lives inside a quoted token (e.g.
        // `...\node_modules\next\dist\bin\next`). Match "next" in any
        // token plus a standalone "dev" token.
        let has_next = command_line.contains("\\next\\")
            || command_line.contains("/next/")
            || tokens.iter().any(|token| token.contains("next"));
        if has_next && (tokens.contains(&"dev") || tokens.contains(&"start")) {
            return (ApplicationKind::NextJs, Confidence::High);
        }
    }

    if is_node(&name) && tokens.iter().any(|token| token.contains("vite")) {
        return (ApplicationKind::Vite, Confidence::High);
    }

    if is_node(&name) {
        return (ApplicationKind::Node, Confidence::Exact);
    }

    if name.starts_with("python") {
        return (ApplicationKind::Python, Confidence::Exact);
    }

    if matches!(
        name.as_str(),
        "postgres.exe" | "postmaster.exe" | "postgres"
    ) {
        return (ApplicationKind::PostgreSql, Confidence::Exact);
    }

    if name.starts_with("redis-server") {
        return (ApplicationKind::Redis, Confidence::Exact);
    }

    if matches!(
        name.as_str(),
        "docker.exe"
            | "dockerd.exe"
            | "dockerd"
            | "com.docker.backend.exe"
            | "docker desktop.exe"
            | "containerd.exe"
            | "containerd"
            | "docker-proxy.exe"
            | "docker-proxy"
    ) {
        return (ApplicationKind::Docker, Confidence::Exact);
    }

    if is_rust_process(process) {
        return (ApplicationKind::Rust, Confidence::Medium);
    }

    (ApplicationKind::Unknown, Confidence::Unknown)
}

fn is_node(name: &str) -> bool {
    name == "node.exe" || name == "node" || name == "nodejs.exe"
}

/// A process is treated as a Rust development binary when it was started by
/// `cargo run` or when its executable lives under a `target/` build tree.
fn is_rust_process(process: &ProcessInfo) -> bool {
    let command_line = process.command_line.as_deref().unwrap_or("");
    if command_line.to_lowercase().contains("cargo run") {
        return true;
    }
    process.executable.as_deref().is_some_and(|path| {
        let path = path.to_string_lossy();
        path.contains("\\target\\") || path.contains("/target/")
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::detect_application;
    use crate::domain::application::ApplicationKind;
    use crate::domain::confidence::Confidence;
    use crate::domain::process::{ProcessId, ProcessInfo};
    use std::path::PathBuf;

    fn process(name: &str, command_line: Option<&str>, executable: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid: ProcessId(1),
            parent_pid: None,
            name: name.to_owned(),
            executable: executable.map(PathBuf::from),
            command_line: command_line.map(str::to_owned),
            working_directory: None,
            started_at: None,
            memory_bytes: None,
            cpu_percent: None,
            ancestors: Vec::new(),
        }
    }

    #[test]
    fn next_dev_is_nextjs() {
        let p = process(
            "node.exe",
            Some(
                r#""C:\Program Files\nodejs\node.exe" "C:\dev\app\node_modules\next\dist\bin\next" dev -p 3000"#,
            ),
            None,
        );
        let (kind, confidence) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::NextJs);
        assert_eq!(confidence, Confidence::High);
    }

    #[test]
    fn next_dev_with_turbo_is_nextjs() {
        let p = process("node.exe", Some("node next dev --turbo"), None);
        let (kind, _) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::NextJs);
    }

    #[test]
    fn vite_is_vite() {
        let p = process(
            "node.exe",
            Some(r#"node "C:\dev\app\node_modules\vite\bin\vite.js" --port 5173"#),
            None,
        );
        let (kind, confidence) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Vite);
        assert_eq!(confidence, Confidence::High);
    }

    #[test]
    fn plain_node_is_node() {
        let p = process("node.exe", Some("node server.js"), None);
        let (kind, confidence) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Node);
        assert_eq!(confidence, Confidence::Exact);
    }

    #[test]
    fn node_without_command_line_is_node() {
        let p = process("node.exe", None, None);
        let (kind, _) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Node);
    }

    #[test]
    fn python_is_python() {
        let p = process("python.exe", Some("python -m http.server"), None);
        let (kind, confidence) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Python);
        assert_eq!(confidence, Confidence::Exact);
    }

    #[test]
    fn postgres_is_postgresql() {
        for name in ["postgres.exe", "postmaster.exe"] {
            let (kind, _) = detect_application(&process(name, None, None));
            assert_eq!(kind, ApplicationKind::PostgreSql, "{name}");
        }
    }

    #[test]
    fn redis_is_redis() {
        let (kind, _) = detect_application(&process("redis-server.exe", None, None));
        assert_eq!(kind, ApplicationKind::Redis);
    }

    #[test]
    fn cargo_run_is_rust() {
        let p = process(
            "server.exe",
            Some("cargo run --release"),
            Some("D:\\dev\\x\\server.exe"),
        );
        let (kind, confidence) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Rust);
        assert_eq!(confidence, Confidence::Medium);
    }

    #[test]
    fn target_debug_binary_is_rust() {
        let p = process(
            "server.exe",
            None,
            Some("D:\\dev\\x\\target\\debug\\server.exe"),
        );
        let (kind, _) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Rust);
    }

    #[test]
    fn dev_token_alone_is_not_nextjs() {
        // "dev" without "next" must not trigger Next.js.
        let p = process("node.exe", Some("node dev-server.js"), None);
        let (kind, _) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Node);
    }

    #[test]
    fn unknown_binary_falls_back_to_unknown() {
        let p = process("weird-tool.exe", Some("--serve"), None);
        let (kind, confidence) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Unknown);
        assert_eq!(confidence, Confidence::Unknown);
    }

    #[test]
    fn matching_is_case_insensitive() {
        let p = process("NODE.EXE", Some("NODE NEXT DEV"), None);
        let (kind, _) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::NextJs);
    }

    #[test]
    fn docker_desktop_is_docker() {
        let p = process("Docker Desktop.exe", None, None);
        let (kind, _) = detect_application(&p);
        assert_eq!(kind, ApplicationKind::Docker);
    }
}
