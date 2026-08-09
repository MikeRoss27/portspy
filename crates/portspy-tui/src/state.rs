//! Application state: the single source of truth for the interface.
//!
//! Widgets render this state; they never mutate it. State changes happen
//! exclusively through [`AppState::apply`].

use portspy_core::domain::listener::ListenerId;
use portspy_core::snapshot::query::{SortField, SortOrder};
use portspy_core::snapshot::SystemSnapshot;
use std::time::{Duration, Instant};

/// What the interface is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Browse,
    Inspect,
    Search,
    ConfirmKill,
}

/// A transient message shown in the footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub message: String,
    shown_at: Instant,
}

impl Notification {
    /// Creates a notification shown for `lifetime`.
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), shown_at: Instant::now() }
    }

    /// Whether the notification has expired.
    pub fn is_expired(&self, lifetime: Duration) -> bool {
        self.shown_at.elapsed() >= lifetime
    }
}

/// The complete interface state.
#[derive(Debug, Clone)]
pub struct AppState {
    pub snapshot: SystemSnapshot,
    /// Stable selection identity — never a row index.
    pub selected: Option<ListenerId>,
    pub mode: AppMode,
    /// Active search query (empty = no filter).
    pub query: String,
    pub sort: SortField,
    pub order: SortOrder,
    pub notification: Option<Notification>,
    /// Pending kill target, set when ConfirmKill is requested.
    pub kill_target: Option<ListenerId>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            snapshot: SystemSnapshot::empty(),
            selected: None,
            mode: AppMode::Browse,
            query: String::new(),
            sort: SortField::Port,
            order: SortOrder::Ascending,
            notification: None,
            kill_target: None,
        }
    }
}

impl AppState {
    /// Creates a fresh state with an empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies an action to the state.
    pub fn apply(&mut self, action: crate::input::actions::Action) {
        match action {
            crate::input::actions::Action::MoveUp => self.move_selection(-1),
            crate::input::actions::Action::MoveDown => self.move_selection(1),
            crate::input::actions::Action::MoveTop => self.select_first(),
            crate::input::actions::Action::MoveBottom => self.select_last(),
            crate::input::actions::Action::Inspect => {
                if self.selected.is_some() {
                    self.mode = AppMode::Inspect;
                }
            }
            crate::input::actions::Action::Back => self.mode = AppMode::Browse,
            crate::input::actions::Action::Search => {
                self.query.clear();
                self.mode = AppMode::Search;
            }
            crate::input::actions::Action::ClearSearch => {
                self.query.clear();
                self.mode = AppMode::Browse;
            }
            crate::input::actions::Action::SortNextField => self.sort = next_sort_field(self.sort),
            crate::input::actions::Action::ToggleSortOrder => {
                self.order = match self.order {
                    SortOrder::Ascending => SortOrder::Descending,
                    SortOrder::Descending => SortOrder::Ascending,
                };
            }
            crate::input::actions::Action::RequestKill => {
                if self.selected.is_some() {
                    self.kill_target = self.selected;
                    self.mode = AppMode::ConfirmKill;
                }
            }
            crate::input::actions::Action::ConfirmKill | crate::input::actions::Action::Cancel => {
                self.kill_target = None;
                self.mode = AppMode::Browse;
            }
            crate::input::actions::Action::Quit => {}
        }
    }

    /// Appends one character to the search query.
    pub fn push_search_char(&mut self, c: char) {
        if self.mode == AppMode::Search {
            self.query.push(c);
        }
    }

    /// Removes the last character of the search query.
    pub fn pop_search_char(&mut self) {
        if self.mode == AppMode::Search {
            self.query.pop();
        }
    }

    /// Moves the selection by `delta` rows in the current view order.
    fn move_selection(&mut self, delta: isize) {
        let rows = self.visible_listeners();
        if rows.is_empty() {
            self.selected = None;
            return;
        }
        let current = self.selected.and_then(|id| rows.iter().position(|l| l.id == id));
        let next = match current {
            Some(index) => index as isize + delta,
            None if delta > 0 => 0,
            None => rows.len() as isize - 1,
        };
        let next = next.clamp(0, rows.len() as isize - 1) as usize;
        self.selected = Some(rows[next].id);
    }

    fn select_first(&mut self) {
        if let Some(first) = self.visible_listeners().first() {
            self.selected = Some(first.id);
        }
    }

    fn select_last(&mut self) {
        if let Some(last) = self.visible_listeners().last() {
            self.selected = Some(last.id);
        }
    }

    /// Reconciles the selection with a fresh snapshot: keeps the stable
    /// selection when the listener survived, otherwise falls back to the
    /// first visible listener (or none).
    pub fn refresh_with(&mut self, snapshot: SystemSnapshot) {
        let survived = self.selected.is_some_and(|id| {
            snapshot.listeners.iter().any(|l| l.id == id)
        });
        self.snapshot = snapshot;
        if survived {
            return;
        }
        self.selected = self.visible_listeners().first().map(|l| l.id);
    }

    /// The listeners visible with the current sort and filter applied.
    pub fn visible_listeners(&self) -> Vec<&portspy_core::domain::listener::Listener> {
        let filtered = portspy_core::snapshot::query::search(&self.snapshot.listeners, &self.query);
        portspy_core::snapshot::query::sort_listener_refs(&filtered, self.sort, self.order)
    }

    /// The currently selected listener, if any.
    pub fn selected_listener(&self) -> Option<&portspy_core::domain::listener::Listener> {
        let id = self.selected?;
        self.snapshot.listeners.iter().find(|l| l.id == id)
    }

    /// The listener targeted for a pending kill.
    pub fn kill_target_listener(&self) -> Option<&portspy_core::domain::listener::Listener> {
        let id = self.kill_target?;
        self.snapshot.listeners.iter().find(|l| l.id == id)
    }

    /// Drops an expired notification.
    pub fn expire_notification(&mut self, lifetime: Duration) {
        if self.notification.as_ref().is_some_and(|n| n.is_expired(lifetime)) {
            self.notification = None;
        }
    }
}

fn next_sort_field(field: SortField) -> SortField {
    match field {
        SortField::Port => SortField::Application,
        SortField::Application => SortField::Origin,
        SortField::Origin => SortField::Memory,
        SortField::Memory => SortField::Uptime,
        SortField::Uptime => SortField::Process,
        SortField::Process => SortField::Port,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::AppState;
    use crate::input::actions::Action;
    use portspy_core::domain::application::{ApplicationInfo, ApplicationKind};
    use portspy_core::domain::confidence::Confidence;
    use portspy_core::domain::listener::{Listener, ListenerId};
    use portspy_core::domain::origin::{OriginInfo, OriginKind};    use portspy_core::domain::process::{ProcessId, ProcessInfo};
    use portspy_core::domain::protocol::Protocol;
    use portspy_core::snapshot::SystemSnapshot;
    use std::net::{IpAddr, Ipv4Addr};

    fn listener(port: u16, pid: u32) -> Listener {
        let process = ProcessInfo {
            pid: ProcessId(pid),
            parent_pid: None,
            name: "node.exe".to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: None,
            memory_bytes: Some(1024),
            cpu_percent: None,
            ancestors: Vec::new(),
        };
        Listener {
            id: ListenerId::new(Protocol::Tcp, IpAddr::V4(Ipv4Addr::LOCALHOST), port, ProcessId(pid), None),
            protocol: Protocol::Tcp,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            process,
            application: ApplicationInfo::new(
                ApplicationKind::Node,
                ApplicationKind::Node.label().to_owned(),
                Confidence::Exact,
            ),
            origin: OriginInfo::new(
                OriginKind::Unknown,
                OriginKind::Unknown.label().to_owned(),
                Confidence::Unknown,
                None,
            ),
        }
    }

    fn state_with(ports: &[u16]) -> AppState {
        let listeners = ports.iter().map(|p| listener(*p, *p as u32 + 100)).collect();
        let mut state = AppState::new();
        state.snapshot = SystemSnapshot {
            captured_at: std::time::Instant::now(),
            wall_time: std::time::SystemTime::now(),
            listeners,
        };
        state
    }

    #[test]
    fn selection_moves_and_wraps() {
        let mut state = state_with(&[3000, 3001, 3002]);
        state.apply(Action::MoveDown);
        let first = state.selected.expect("selected");
        assert_eq!(first.port, 3000);
        state.apply(Action::MoveDown);
        assert_eq!(state.selected.expect("selected").port, 3001);
        state.apply(Action::MoveDown);
        assert_eq!(state.selected.expect("selected").port, 3002);
        state.apply(Action::MoveDown);
        assert_eq!(state.selected.expect("selected").port, 3002);
        state.apply(Action::MoveUp);
        assert_eq!(state.selected.expect("selected").port, 3001);
    }

    #[test]
    fn selection_is_stable_across_snapshot_refresh() {
        let mut state = state_with(&[3000, 3001]);
        state.apply(Action::MoveDown);
        state.apply(Action::MoveDown);
        let stable_id = state.selected.expect("selected");

        // A refresh that keeps the listener must keep the selection.
        let mut fresh = state.snapshot.clone();
        fresh.listeners.push(listener(5173, 42));
        state.refresh_with(fresh);
        assert_eq!(state.selected, Some(stable_id));
    }

    #[test]
    fn selection_falls_back_to_first_when_listener_dies() {
        let mut state = state_with(&[3000, 3001]);
        state.apply(Action::MoveDown);
        state.apply(Action::MoveDown);

        let mut fresh = state.snapshot.clone();
        fresh.listeners.retain(|l| l.port == 3001);
        state.refresh_with(fresh);
        assert_eq!(state.selected.expect("selected").port, 3001);
    }

    #[test]
    fn selection_falls_back_to_none_on_empty_snapshot() {
        let mut state = state_with(&[3000]);
        state.apply(Action::MoveDown);
        let fresh = SystemSnapshot {
            captured_at: std::time::Instant::now(),
            wall_time: std::time::SystemTime::now(),
            listeners: vec![],
        };
        state.refresh_with(fresh);
        assert_eq!(state.selected, None);
    }

    #[test]
    fn search_filters_visible_listeners() {
        let mut state = state_with(&[3000, 5173, 8080]);
        state.query = "5173".to_owned();
        let visible = state.visible_listeners();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].port, 5173);
    }

    #[test]
    fn mode_transitions_are_guarded() {
        let mut state = state_with(&[3000]);
        // Inspect requires a selection.
        state.apply(Action::Inspect);
        assert_eq!(state.mode, super::AppMode::Browse);
        state.apply(Action::MoveDown);
        state.apply(Action::Inspect);
        assert_eq!(state.mode, super::AppMode::Inspect);
        state.apply(Action::Back);
        assert_eq!(state.mode, super::AppMode::Browse);
    }

    #[test]
    fn kill_confirmation_round_trip() {
        let mut state = state_with(&[3000]);
        state.apply(Action::MoveDown);
        state.apply(Action::RequestKill);
        assert_eq!(state.mode, super::AppMode::ConfirmKill);
        assert_eq!(state.kill_target, state.selected);
        state.apply(Action::Cancel);
        assert_eq!(state.mode, super::AppMode::Browse);
        assert_eq!(state.kill_target, None);
    }

    #[test]
    fn sort_cycles_through_fields() {
        let mut state = state_with(&[3000]);
        let mut seen = Vec::new();
        for _ in 0..6 {
            seen.push(state.sort);
            state.apply(Action::SortNextField);
        }
        assert_eq!(seen[0], state.sort);
        assert_ne!(seen[0], seen[1]);
    }
}
