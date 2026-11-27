//! Duplicate dev-server detection.
//!
//! Two *different processes* serving the same project at once is almost
//! always a mistake: a forgotten `next dev` from a second terminal, an IDE
//! auto-start alongside a manual start. This engine marks every listener of
//! such a project as a duplicate.
//!
//! # Rules
//!
//! * Only dev-server applications count (Next.js, Vite, Node, Python, Rust).
//!   Two PostgreSQL instances on one project can be intentional.
//! * Duplicates require *distinct owning processes*: one process holding
//!   several ports is one server, not a duplicate.
//! * The project must be known; two servers with no detectable project are
//!   not flagged.
//! * Two processes with different command lines are considered different
//!   services (e.g., `next dev` vs `next start`), not duplicates.

use std::collections::{BTreeSet, HashMap};

use crate::domain::application::ApplicationKind;
use crate::domain::listener::Listener;
use crate::domain::process::ProcessId;
use crate::domain::project::ProjectKind;

/// Applications that are plausible dev servers.
fn is_dev_server(kind: ApplicationKind) -> bool {
    matches!(
        kind,
        ApplicationKind::NextJs
            | ApplicationKind::Vite
            | ApplicationKind::Node
            | ApplicationKind::Python
            | ApplicationKind::Rust
    )
}

/// Extracts a stable command signature from a command line for duplicate detection.
/// Returns None if no command line is available.
fn command_signature(cmd: &Option<String>) -> Option<String> {
    cmd.as_ref().map(|c| {
        // Normalize: take first 3 tokens (executable + main args), lowercase
        let tokens: Vec<&str> = c.split_whitespace().take(3).collect();
        tokens.join(" ").to_lowercase()
    })
}

/// Marks every listener whose project is served by two or more processes
/// running the same command signature.
///
/// The flags are recomputed from scratch on every call, so the function is
/// idempotent: running it twice never changes a result.
pub(crate) fn mark_duplicates(listeners: &mut [Listener]) {
    let mut owners_by_project: HashMap<
        (String, ApplicationKind, Option<String>),
        BTreeSet<ProcessId>,
    > = HashMap::new();

    // First pass: collect owners per project+app kind+command signature
    for listener in listeners.iter() {
        let ProjectKind::Named(project) = &listener.project.kind else {
            continue;
        };
        if is_dev_server(listener.application.kind) {
            let sig = command_signature(&listener.process.command_line);
            owners_by_project
                .entry((project.clone(), listener.application.kind, sig))
                .or_default()
                .insert(listener.pid());
        }
    }

    let duplicated_projects: BTreeSet<(String, ApplicationKind, Option<String>)> =
        owners_by_project
            .iter()
            .filter(|(_, owners)| owners.len() > 1)
            .map(|(key, _)| key.clone())
            .collect();

    // Second pass: mark duplicates
    for listener in listeners.iter_mut() {
        let is_duplicate = match &listener.project.kind {
            ProjectKind::Named(project) => {
                let sig = command_signature(&listener.process.command_line);
                duplicated_projects.contains(&(project.clone(), listener.application.kind, sig))
            }
            ProjectKind::Unknown => false,
        };
        listener.is_duplicate = is_duplicate;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::mark_duplicates;
    use crate::domain::application::{ApplicationInfo, ApplicationKind};
    use crate::domain::confidence::Confidence;
    use crate::domain::listener::{Listener, ListenerId};
    use crate::domain::origin::{OriginInfo, OriginKind};
    use crate::domain::process::{ProcessId, ProcessInfo};
    use crate::domain::project::{ProjectInfo, ProjectKind};
    use crate::domain::protocol::Protocol;
    use std::net::{IpAddr, Ipv4Addr};

    fn listener(id: u32, project: Option<&str>, app: ApplicationKind) -> Listener {
        let project_kind = project.map_or(ProjectKind::Unknown, |name| {
            ProjectKind::Named(name.to_owned())
        });
        Listener {
            id: ListenerId::new(
                Protocol::Tcp,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                3000 + id as u16,
                ProcessId(id),
                None,
            ),
            protocol: Protocol::Tcp,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 3000 + id as u16,
            socket_attribution: None,
            process: ProcessInfo {
                pid: ProcessId(id),
                parent_pid: None,
                name: "node.exe".to_owned(),
                executable: None,
                command_line: None,
                working_directory: None,
                started_at: None,
                memory_bytes: None,
                cpu_percent: None,
                ancestors: Vec::new(),
            },
            application: ApplicationInfo::new(app, app.label().to_owned(), Confidence::Exact),
            origin: OriginInfo::new(
                OriginKind::Unknown,
                OriginKind::Unknown.label().to_owned(),
                Confidence::Unknown,
                None,
            ),
            project: ProjectInfo::new(project_kind, Confidence::High),
            is_duplicate: false,
            is_orphan: false,
        }
    }

    #[test]
    fn two_processes_on_same_project_are_duplicates() {
        let mut listeners = vec![
            listener(1, Some("bengal"), ApplicationKind::NextJs),
            listener(2, Some("bengal"), ApplicationKind::NextJs),
        ];
        mark_duplicates(&mut listeners);
        assert!(listeners[0].is_duplicate);
        assert!(listeners[1].is_duplicate);
    }

    #[test]
    fn one_process_is_not_a_duplicate() {
        let mut listeners = vec![listener(1, Some("bengal"), ApplicationKind::NextJs)];
        mark_duplicates(&mut listeners);
        assert!(!listeners[0].is_duplicate);
    }

    #[test]
    fn one_process_holding_two_ports_is_not_a_duplicate() {
        let a = listener(1, Some("bengal"), ApplicationKind::NextJs);
        let mut b = listener(1, Some("bengal"), ApplicationKind::NextJs);
        b.port = 3001;
        let mut listeners = vec![a, b];
        mark_duplicates(&mut listeners);
        assert!(!listeners[0].is_duplicate);
        assert!(!listeners[1].is_duplicate);
    }

    #[test]
    fn different_projects_are_not_duplicates() {
        let mut listeners = vec![
            listener(1, Some("bengal"), ApplicationKind::NextJs),
            listener(2, Some("lumen"), ApplicationKind::NextJs),
        ];
        mark_duplicates(&mut listeners);
        assert!(!listeners[0].is_duplicate);
        assert!(!listeners[1].is_duplicate);
    }

    #[test]
    fn infra_applications_are_not_duplicates() {
        let mut listeners = vec![
            listener(1, Some("bengal"), ApplicationKind::PostgreSql),
            listener(2, Some("bengal"), ApplicationKind::PostgreSql),
        ];
        mark_duplicates(&mut listeners);
        assert!(!listeners[0].is_duplicate);
        assert!(!listeners[1].is_duplicate);
    }

    #[test]
    fn unknown_project_is_not_a_duplicate() {
        let mut listeners = vec![
            listener(1, None, ApplicationKind::NextJs),
            listener(2, None, ApplicationKind::NextJs),
        ];
        mark_duplicates(&mut listeners);
        assert!(!listeners[0].is_duplicate);
        assert!(!listeners[1].is_duplicate);
    }

    #[test]
    fn duplicate_flag_survives_repeat_calls() {
        let mut listeners = vec![
            listener(1, Some("bengal"), ApplicationKind::Vite),
            listener(2, Some("bengal"), ApplicationKind::Vite),
        ];
        mark_duplicates(&mut listeners);
        mark_duplicates(&mut listeners);
        assert!(listeners[0].is_duplicate);
        assert!(listeners[1].is_duplicate);
    }
}
