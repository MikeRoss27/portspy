//! Project detection rules.
//!
//! Detection priority:
//!
//! ```text
//! working directory project name  →  unknown
//! ```
//!
//! The working directory is the strongest fact about project membership: a
//! process whose cwd is `D:\dev\bengal` is almost certainly part of the
//! `bengal` project. Non-project folders (system directories, `node_modules`,
//! build trees, user homes) yield `Unknown`.

use crate::domain::confidence::Confidence;
use crate::domain::process::ProcessInfo;
use crate::domain::project::ProjectKind;

/// Folder names that never identify a project, compared case-insensitively.
const NON_PROJECT_FOLDERS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "out",
    "bin",
    "obj",
    "src",
    "test",
    "tests",
    "windows",
    "system32",
    "syswow64",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "local",
    "roaming",
    "temp",
    "tmp",
    "users",
    "public",
    "default",
    "etc",
    "usr",
    "opt",
    "var",
    "home",
    "root",
    "lib",
    "share",
    ".cargo",
    ".git",
    ".vscode",
    ".idea",
    "$recycle.bin",
    "recovery",
    "perflogs",
];

/// Detects the project of a process from its working directory.
///
/// The project name is the last path segment of the working directory unless
/// that segment is a known non-project folder. A missing working directory
/// yields `Unknown`; so does a directory directly under a user's home
/// (`C:\Users\alice` is a user, not a project).
pub(crate) fn detect_project(process: &ProcessInfo) -> (ProjectKind, Confidence) {
    let Some(cwd) = &process.working_directory else {
        return (ProjectKind::Unknown, Confidence::Unknown);
    };

    let Some(name) = last_segment(cwd) else {
        return (ProjectKind::Unknown, Confidence::Unknown);
    };

    let lower = name.to_lowercase();
    if NON_PROJECT_FOLDERS.contains(&lower.as_str()) {
        return (ProjectKind::Unknown, Confidence::Unknown);
    }

    if is_home_directory(cwd) {
        return (ProjectKind::Unknown, Confidence::Unknown);
    }

    (ProjectKind::Named(name), Confidence::High)
}

/// Last path segment on either separator, trimmed of trailing separators.
///
/// Working directories are Windows paths, but detection must also be testable
/// on Linux where `Path::file_name` does not split backslashes. Splitting on
/// both separators keeps the rules platform-independent.
fn last_segment(cwd: &std::path::Path) -> Option<String> {
    let trimmed = cwd.to_string_lossy();
    let trimmed = trimmed.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        return None;
    }
    trimmed.rsplit(['/', '\\']).next().map(str::to_owned)
}

/// A directory directly under `Users`, `home`, or `root` is a user home, not a project.
fn is_home_directory(cwd: &std::path::Path) -> bool {
    let trimmed = cwd.to_string_lossy();
    let trimmed = trimmed.trim_end_matches(['/', '\\']);
    let mut segments = trimmed.rsplit(['/', '\\']);
    let _last = segments.next();
    segments.next().is_some_and(|parent| {
        parent.eq_ignore_ascii_case("users")
            || parent.eq_ignore_ascii_case("home")
            || parent.eq_ignore_ascii_case("root")
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::detect_project;
    use crate::domain::confidence::Confidence;
    use crate::domain::process::{ProcessId, ProcessInfo};
    use crate::domain::project::ProjectKind;
    use std::path::PathBuf;

    fn process(cwd: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid: ProcessId(1),
            parent_pid: None,
            name: "node.exe".to_owned(),
            executable: None,
            command_line: None,
            working_directory: cwd.map(PathBuf::from),
            started_at: None,
            memory_bytes: None,
            cpu_percent: None,
            ancestors: Vec::new(),
        }
    }

    #[test]
    fn dev_folder_is_a_project() {
        let (kind, confidence) = detect_project(&process(Some(r"D:\dev\bengal")));
        assert_eq!(kind, ProjectKind::Named("bengal".to_owned()));
        assert_eq!(confidence, Confidence::High);
    }

    #[test]
    fn trailing_slash_does_not_break_detection() {
        let (kind, _) = detect_project(&process(Some(r"D:\dev\bengal\")));
        assert_eq!(kind, ProjectKind::Named("bengal".to_owned()));
    }

    #[test]
    fn forward_slash_paths_work() {
        let (kind, _) = detect_project(&process(Some("/home/alice/work/portspy")));
        assert_eq!(kind, ProjectKind::Named("portspy".to_owned()));
    }

    #[test]
    fn missing_working_directory_is_unknown() {
        let (kind, confidence) = detect_project(&process(None));
        assert_eq!(kind, ProjectKind::Unknown);
        assert_eq!(confidence, Confidence::Unknown);
    }

    #[test]
    fn drive_root_is_unknown() {
        let (kind, _) = detect_project(&process(Some(r"C:\")));
        assert_eq!(kind, ProjectKind::Unknown);
    }

    #[test]
    fn node_modules_is_not_a_project() {
        let (kind, _) = detect_project(&process(Some(r"D:\dev\bengal\node_modules")));
        assert_eq!(kind, ProjectKind::Unknown);
    }

    #[test]
    fn system32_is_not_a_project() {
        let (kind, _) = detect_project(&process(Some(r"C:\Windows\System32")));
        assert_eq!(kind, ProjectKind::Unknown);
    }

    #[test]
    fn user_home_is_not_a_project() {
        let (kind, _) = detect_project(&process(Some(r"C:\Users\alice")));
        assert_eq!(kind, ProjectKind::Unknown);
    }

    #[test]
    fn folder_name_matching_is_case_insensitive() {
        let (kind, _) = detect_project(&process(Some(r"D:\dev\app\Node_Modules")));
        assert_eq!(kind, ProjectKind::Unknown);
    }

    #[test]
    fn empty_path_is_unknown() {
        let (kind, _) = detect_project(&process(Some("")));
        assert_eq!(kind, ProjectKind::Unknown);
    }
}
