//! Origin detection rules.
//!
//! Rules are declarative data: adding a new agent later means appending an
//! entry to [`ORIGIN_RULES`] — no engine change.

use crate::detection::DetectionContext;
use crate::domain::confidence::Confidence;
use crate::domain::origin::{OriginInfo, OriginKind};
use crate::domain::process::ProcessInfo;
use crate::processes::ancestry::ProcessAncestry;

/// A declarative origin rule.
#[derive(Debug, Clone, Copy)]
struct OriginRule {
    kind: OriginKind,
    /// Executable names that match this origin, all lowercase.
    executable_names: &'static [&'static str],
}

impl OriginRule {
    fn matches(self, name: &str) -> bool {
        self.executable_names.contains(&name)
    }
}

/// Rules ordered from most specific to least specific: the first match wins.
///
/// AI agents beat IDEs, IDEs beat terminals, terminals beat the environment.
/// `wslrelay.exe` is a generic per-vm helper, so WSL sits last among
/// environments; `bash.exe` is deliberately absent (Git Bash would be a
/// false positive).
const ORIGIN_RULES: &[OriginRule] = &[
    OriginRule {
        kind: OriginKind::Codex,
        executable_names: &["codex.exe", "codex"],
    },
    OriginRule {
        kind: OriginKind::ClaudeCode,
        executable_names: &["claude.exe", "claude"],
    },
    OriginRule {
        kind: OriginKind::Cursor,
        executable_names: &["cursor.exe"],
    },
    OriginRule {
        kind: OriginKind::Zed,
        executable_names: &["zed.exe"],
    },
    OriginRule {
        kind: OriginKind::VsCode,
        executable_names: &["code.exe"],
    },
    OriginRule {
        kind: OriginKind::WindowsTerminal,
        executable_names: &["windowsterminal.exe", "wt.exe", "wt"],
    },
    OriginRule {
        kind: OriginKind::PowerShell,
        executable_names: &["powershell.exe", "pwsh.exe", "windowspowershell.exe"],
    },
    OriginRule {
        kind: OriginKind::Cmd,
        executable_names: &["cmd.exe"],
    },
    OriginRule {
        kind: OriginKind::Docker,
        executable_names: &[
            "docker desktop.exe",
            "docker.exe",
            "com.docker.backend.exe",
            "dockerd.exe",
            "containerd.exe",
            "docker-proxy.exe",
            "docker-proxy",
        ],
    },
    OriginRule {
        kind: OriginKind::Wsl,
        executable_names: &["wsl.exe", "wslhost.exe", "wslrelay.exe"],
    },
];

/// Detects the origin of `process` given its ancestor chain.
///
/// The leaf process itself is checked first (a match there is [`Confidence::Exact`]),
/// then the ancestor chain from nearest to farthest (matches are
/// [`Confidence::High`]). No match yields [`OriginKind::Unknown`].
pub(crate) fn detect_origin(
    _context: &DetectionContext<'_>,
    process: &ProcessInfo,
    ancestry: &ProcessAncestry,
) -> OriginInfo {
    let leaf_name = process.name.to_lowercase();
    if let Some(rule) = ORIGIN_RULES.iter().find(|rule| rule.matches(&leaf_name)) {
        return OriginInfo::new(
            rule.kind,
            rule.kind.label().to_owned(),
            Confidence::Exact,
            Some(process.pid),
        );
    }

    // Ancestors: rule priority beats chain distance. A chain containing both
    // Codex and Claude Code resolves to Codex, because agents are matched in
    // priority order (Codex before Claude Code, agents before IDEs, ...).
    for rule in ORIGIN_RULES {
        if let Some(ancestor) = ancestry
            .chain
            .iter()
            .find(|ancestor| rule.matches(&ancestor.name.to_lowercase()))
        {
            return OriginInfo::new(
                rule.kind,
                rule.kind.label().to_owned(),
                Confidence::High,
                Some(ancestor.identity.pid),
            );
        }
    }

    OriginInfo::default()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{ORIGIN_RULES, detect_origin};
    use crate::detection::DetectionContext;
    use crate::domain::confidence::Confidence;
    use crate::domain::origin::{OriginInfo, OriginKind};
    use crate::domain::process::{ProcessId, ProcessInfo};
    use crate::processes::ancestry::AncestryBuilder;
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    fn process(pid: u32, parent: Option<u32>, name: &str) -> ProcessInfo {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(u64::from(pid) * 10);
        ProcessInfo {
            pid: ProcessId(pid),
            parent_pid: parent.map(ProcessId),
            name: name.to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: Some(start),
            memory_bytes: None,
            cpu_percent: None,
            ancestors: Vec::new(),
        }
    }

    /// Builds the fixture chain `WindowsTerminal(10) → pwsh(11) → agent(12) → node(13)`.
    fn chain_with_agent(agent_name: &str, agent_pid: u32) -> HashMap<ProcessId, ProcessInfo> {
        let tree = vec![
            process(10, None, "WindowsTerminal.exe"),
            process(11, Some(10), "pwsh.exe"),
            process(agent_pid, Some(11), agent_name),
            process(13, Some(agent_pid), "node.exe"),
        ];
        tree.into_iter().map(|p| (p.pid, p)).collect()
    }

    fn detect_in_chain(processes: &HashMap<ProcessId, ProcessInfo>) -> OriginInfo {
        let builder = AncestryBuilder::new(processes.values().cloned());
        let ctx = DetectionContext::new(processes, &builder);
        let leaf = processes.get(&ProcessId(13)).expect("leaf present");
        let ancestry = ctx.ancestry(ProcessId(13));
        detect_origin(&ctx, leaf, &ancestry)
    }

    fn leaf_only(name: &str) -> OriginInfo {
        let processes: HashMap<_, _> = [(ProcessId(1), process(1, None, name))]
            .into_iter()
            .collect();
        let builder = AncestryBuilder::new(processes.values().cloned());
        let ctx = DetectionContext::new(&processes, &builder);
        let leaf = processes.get(&ProcessId(1)).expect("leaf present");
        let ancestry = ctx.ancestry(ProcessId(1));
        detect_origin(&ctx, leaf, &ancestry)
    }

    #[test]
    fn codex_in_chain_wins_over_terminal() {
        let processes = chain_with_agent("codex.exe", 12);
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::Codex);
        assert_eq!(origin.confidence, Confidence::High);
        assert_eq!(origin.matched_pid, Some(ProcessId(12)));
    }

    #[test]
    fn claude_code_in_chain_wins_over_terminal() {
        let processes = chain_with_agent("claude.exe", 12);
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::ClaudeCode);
        assert_eq!(origin.matched_pid, Some(ProcessId(12)));
    }

    #[test]
    fn cursor_in_chain_wins_over_terminal() {
        let processes = chain_with_agent("Cursor.exe", 12);
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::Cursor);
        assert_eq!(origin.matched_pid, Some(ProcessId(12)));
    }

    #[test]
    fn vscode_in_chain_wins_over_terminal() {
        let processes = chain_with_agent("Code.exe", 12);
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::VsCode);
        assert_eq!(origin.matched_pid, Some(ProcessId(12)));
    }

    #[test]
    fn zed_in_chain_wins_over_terminal() {
        let processes = chain_with_agent("Zed.exe", 12);
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::Zed);
        assert_eq!(origin.matched_pid, Some(ProcessId(12)));
    }

    #[test]
    fn unrelated_zed_name_does_not_match() {
        let origin = leaf_only("zed-helper.exe");
        assert_eq!(origin.kind, OriginKind::Unknown);
    }

    #[test]
    fn powershell_chain_resolves_to_powershell() {
        let processes = chain_with_agent("powershell.exe", 11);
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::PowerShell);
        assert_eq!(origin.matched_pid, Some(ProcessId(11)));
    }

    #[test]
    fn bare_terminal_chain_resolves_to_windows_terminal() {
        let processes: HashMap<_, _> = [
            (ProcessId(10), process(10, None, "WindowsTerminal.exe")),
            (ProcessId(13), process(13, Some(10), "node.exe")),
        ]
        .into_iter()
        .collect();
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::WindowsTerminal);
        assert_eq!(origin.confidence, Confidence::High);
    }

    #[test]
    fn cmd_is_detected() {
        let processes: HashMap<_, _> = [
            (ProcessId(10), process(10, None, "cmd.exe")),
            (ProcessId(13), process(13, Some(10), "python.exe")),
        ]
        .into_iter()
        .collect();
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::Cmd);
    }

    #[test]
    fn docker_backend_is_detected() {
        let processes: HashMap<_, _> = [
            (ProcessId(10), process(10, None, "com.docker.backend.exe")),
            (ProcessId(13), process(13, Some(10), "postgres.exe")),
        ]
        .into_iter()
        .collect();
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::Docker);
    }

    #[test]
    fn wslrelay_is_detected_as_wsl() {
        let processes: HashMap<_, _> = [
            (ProcessId(10), process(10, None, "wslrelay.exe")),
            (ProcessId(13), process(13, Some(10), "node.exe")),
        ]
        .into_iter()
        .collect();
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::Wsl);
    }

    #[test]
    fn leaf_match_is_exact() {
        let origin = leaf_only("codex.exe");
        assert_eq!(origin.kind, OriginKind::Codex);
        assert_eq!(origin.confidence, Confidence::Exact);
        assert_eq!(origin.matched_pid, Some(ProcessId(1)));
    }

    #[test]
    fn no_agent_resolves_to_unknown() {
        let processes: HashMap<_, _> = [
            (ProcessId(10), process(10, None, "explorer.exe")),
            (ProcessId(13), process(13, Some(10), "node.exe")),
        ]
        .into_iter()
        .collect();
        let origin = detect_in_chain(&processes);
        assert_eq!(origin.kind, OriginKind::Unknown);
        assert_eq!(origin.confidence, Confidence::Unknown);
        assert_eq!(origin.matched_pid, None);
    }

    #[test]
    fn matching_is_case_insensitive() {
        let origin = leaf_only("CLAUDE.EXE");
        assert_eq!(origin.kind, OriginKind::ClaudeCode);
        assert_eq!(origin.confidence, Confidence::Exact);
    }

    #[test]
    fn codex_wins_over_claude_when_both_present() {
        let processes: HashMap<_, _> = [
            (ProcessId(10), process(10, None, "WindowsTerminal.exe")),
            (ProcessId(11), process(11, Some(10), "codex.exe")),
            (ProcessId(12), process(12, Some(11), "claude.exe")),
            (ProcessId(13), process(13, Some(12), "node.exe")),
        ]
        .into_iter()
        .collect();
        let origin = detect_in_chain(&processes);
        // Rule priority beats distance: Codex is checked before Claude Code.
        assert_eq!(origin.kind, OriginKind::Codex);
        assert_eq!(origin.matched_pid, Some(ProcessId(11)));
    }

    #[test]
    fn every_rule_matches_its_own_exe_name() {
        let expected: Vec<(OriginKind, &str)> = vec![
            (OriginKind::Codex, "codex.exe"),
            (OriginKind::Codex, "codex"),
            (OriginKind::ClaudeCode, "claude.exe"),
            (OriginKind::ClaudeCode, "claude"),
            (OriginKind::Cursor, "cursor.exe"),
            (OriginKind::Zed, "Zed.exe"),
            (OriginKind::VsCode, "Code.exe"),
            (OriginKind::WindowsTerminal, "WindowsTerminal.exe"),
            (OriginKind::PowerShell, "pwsh.exe"),
            (OriginKind::PowerShell, "powershell.exe"),
            (OriginKind::Cmd, "cmd.exe"),
            (OriginKind::Docker, "Docker Desktop.exe"),
            (OriginKind::Docker, "docker.exe"),
            (OriginKind::Wsl, "wsl.exe"),
            (OriginKind::Wsl, "wslrelay.exe"),
        ];
        for (kind, name) in expected {
            let origin = leaf_only(name);
            assert_eq!(origin.kind, kind, "rule for {name} should map to {kind:?}");
            assert_eq!(origin.confidence, Confidence::Exact);
        }
    }

    #[test]
    fn git_bash_is_not_wsl() {
        // bash.exe is not in the WSL rule set on purpose.
        assert!(
            !ORIGIN_RULES
                .iter()
                .find(|r| r.kind == OriginKind::Wsl)
                .is_some_and(|r| r.executable_names.contains(&"bash.exe"))
        );
    }
}
