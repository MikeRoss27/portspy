//! Application state: the single source of truth for the interface.
//!
//! Widgets render this state; they never mutate it. State changes happen
//! exclusively through [`AppState::apply`].

use portspy_core::domain::listener::ListenerId;
use portspy_core::processes::stop::StopPlan;
use portspy_core::processes::tree::TreeKillReport;
use portspy_core::snapshot::SystemSnapshot;
use portspy_core::snapshot::query::{SortField, SortOrder};
use std::time::{Duration, Instant};

use crate::recent::RecentListeners;

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
        Self {
            message: message.into(),
            shown_at: Instant::now(),
        }
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
    /// First visible line of the inspection pane.
    pub inspection_scroll: u16,
    pub(crate) inspection_page_size: u16,
    /// Active search query (empty = no filter).
    pub query: String,
    recent_only: bool,
    known_owners_only: bool,
    recent: RecentListeners,
    pub sort: SortField,
    pub order: SortOrder,
    pub notification: Option<Notification>,
    /// Scope displayed for a pending stop confirmation.
    pub stop_plan: Option<StopPlan>,
    /// First visible line of the stop review.
    pub stop_review_scroll: u16,
    stop_in_progress: bool,
    pending_stop_feedback: Option<StopFeedback>,
    stop_return_mode: AppMode,
    pub table_state: ratatui::widgets::TableState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            snapshot: SystemSnapshot::empty(),
            selected: None,
            mode: AppMode::Browse,
            inspection_scroll: 0,
            inspection_page_size: 1,
            query: String::new(),
            recent_only: false,
            known_owners_only: false,
            recent: RecentListeners::default(),
            sort: SortField::Port,
            order: SortOrder::Ascending,
            notification: None,
            stop_plan: None,
            stop_review_scroll: 0,
            stop_in_progress: false,
            pending_stop_feedback: None,
            stop_return_mode: AppMode::Browse,
            table_state: ratatui::widgets::TableState::default(),
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
                if self.selected.is_some() && self.mode != AppMode::Inspect {
                    self.mode = AppMode::Inspect;
                    self.inspection_scroll = 0;
                }
            }
            crate::input::actions::Action::Back => {
                if self.mode == AppMode::Browse {
                    self.clear_search();
                }
                self.mode = AppMode::Browse;
            }
            crate::input::actions::Action::Search => self.mode = AppMode::Search,
            crate::input::actions::Action::ClearSearch => {
                self.clear_search();
                self.mode = AppMode::Browse;
            }
            crate::input::actions::Action::ToggleRecent => {
                self.recent_only = !self.recent_only;
                self.mode = AppMode::Browse;
                self.reconcile_selection();
            }
            crate::input::actions::Action::ToggleKnownOwners => {
                self.known_owners_only = !self.known_owners_only;
                self.mode = AppMode::Browse;
                self.reconcile_selection();
            }
            crate::input::actions::Action::SortNextField => {
                self.sort = next_sort_field(self.sort);
                self.reconcile_selection();
            }
            crate::input::actions::Action::ToggleSortOrder => {
                self.order = match self.order {
                    SortOrder::Ascending => SortOrder::Descending,
                    SortOrder::Descending => SortOrder::Ascending,
                };
                self.reconcile_selection();
            }
            crate::input::actions::Action::RequestKill => self.request_stop(false),
            crate::input::actions::Action::RequestKillTree => self.request_stop(true),
            crate::input::actions::Action::ConfirmKill | crate::input::actions::Action::Cancel => {
                self.close_stop_review();
            }
            crate::input::actions::Action::ScrollDetailsUp if self.mode == AppMode::ConfirmKill => {
                self.stop_review_scroll = self.stop_review_scroll.saturating_sub(1);
            }
            crate::input::actions::Action::ScrollDetailsDown
                if self.mode == AppMode::ConfirmKill =>
            {
                self.stop_review_scroll = self.stop_review_scroll.saturating_add(1);
            }
            crate::input::actions::Action::ScrollDetailsUp if self.mode == AppMode::Inspect => {
                self.inspection_scroll = self
                    .inspection_scroll
                    .saturating_sub(self.inspection_page_size);
            }
            crate::input::actions::Action::ScrollDetailsDown if self.mode == AppMode::Inspect => {
                self.inspection_scroll = self
                    .inspection_scroll
                    .saturating_add(self.inspection_page_size);
            }
            crate::input::actions::Action::ScrollDetailsUp
            | crate::input::actions::Action::ScrollDetailsDown
            | crate::input::actions::Action::Quit => {}
        }
    }

    fn request_stop(&mut self, tree: bool) {
        if self.stop_in_progress {
            return;
        }
        let Some(selected) = self.selected else {
            return;
        };
        match StopPlan::review(&self.snapshot, selected, tree) {
            Ok(plan) => {
                self.stop_plan = Some(plan);
                self.stop_review_scroll = 0;
                self.stop_return_mode = self.mode;
                self.mode = AppMode::ConfirmKill;
            }
            Err(error) => {
                self.notification = Some(Notification::new(format!("Cannot stop: {error}")));
            }
        }
    }

    fn close_stop_review(&mut self) {
        if self.mode == AppMode::ConfirmKill {
            self.stop_plan = None;
            self.mode = self.stop_return_mode;
        }
    }

    pub(crate) const fn mode_behind_stop_review(&self) -> AppMode {
        self.stop_return_mode
    }

    pub(crate) fn cancel_stop(&mut self, reason: impl Into<String>) {
        self.close_stop_review();
        self.notification = Some(Notification::new(reason));
    }

    pub(crate) fn begin_stop(&mut self) -> Option<StopPlan> {
        if self.stop_in_progress || self.mode != AppMode::ConfirmKill {
            return None;
        }
        let plan = self.stop_plan.take()?;
        self.apply(crate::input::actions::Action::ConfirmKill);
        self.stop_in_progress = true;
        Some(plan)
    }

    pub(crate) const fn stop_in_progress(&self) -> bool {
        self.stop_in_progress
    }

    pub(crate) fn stop_validation_failed(&mut self, reason: impl Into<String>) {
        self.stop_in_progress = false;
        self.cancel_stop(reason);
    }

    pub(crate) fn report_stop(&mut self, plan: StopPlan, report: TreeKillReport) {
        self.stop_in_progress = false;
        let outcome = if plan.tree {
            report.failed.first().map_or_else(
                || report.summary(),
                |(_, error)| format!("{} ({error})", report.summary()),
            )
        } else if let Some((_, error)) = report.failed.first() {
            format!("Stop failed: {error}")
        } else if report.terminated.is_empty() {
            format!("PID {} already exited", plan.process.identity.pid)
        } else {
            format!("Stopped PID {}", plan.process.identity.pid)
        };
        let should_verify = !report.terminated.is_empty() || !report.already_exited.is_empty();
        self.pending_stop_feedback = should_verify.then(|| StopFeedback {
            plan,
            outcome: outcome.clone(),
            after: Instant::now(),
        });
        let message = if should_verify {
            format!("{outcome} · checking ports")
        } else {
            outcome
        };
        self.notification = Some(Notification::new(message));
    }

    /// Appends one character to the search query.
    pub fn push_search_char(&mut self, c: char) {
        if self.mode == AppMode::Search {
            self.query.push(c);
            self.reconcile_selection();
        }
    }

    /// Removes the last character of the search query.
    pub fn pop_search_char(&mut self) {
        if self.mode == AppMode::Search {
            self.query.pop();
            self.reconcile_selection();
        }
    }

    fn clear_search(&mut self) {
        self.query.clear();
        self.reconcile_selection();
    }

    /// Moves the selection by `delta` rows in the current view order.
    fn move_selection(&mut self, delta: isize) {
        let rows = self.visible_listeners();
        if rows.is_empty() {
            self.set_selection(None);
            return;
        }
        let current = self
            .selected
            .and_then(|id| rows.iter().position(|l| l.id == id));
        let next = match current {
            Some(index) => index as isize + delta,
            None if delta > 0 => 0,
            None => rows.len() as isize - 1,
        };
        let next = next.clamp(0, rows.len() as isize - 1) as usize;
        self.set_selection(Some((rows[next].id, next)));
    }

    fn select_first(&mut self) {
        let first = self
            .visible_listeners()
            .first()
            .map(|listener| (listener.id, 0));
        self.set_selection(first);
    }

    fn select_last(&mut self) {
        let rows = self.visible_listeners();
        let last = rows.last().map(|listener| (listener.id, rows.len() - 1));
        self.set_selection(last);
    }

    fn set_selection(&mut self, selection: Option<(ListenerId, usize)>) {
        let selected = selection.map(|(id, _)| id);
        if self.selected != selected {
            self.inspection_scroll = 0;
        }
        self.selected = selected;
        self.table_state.select(selection.map(|(_, index)| index));
    }

    fn reconcile_selection(&mut self) {
        let rows = self.visible_listeners();
        let selection = rows
            .iter()
            .enumerate()
            .find(|(_, listener)| self.selected == Some(listener.id))
            .or_else(|| rows.first().map(|listener| (0, listener)))
            .map(|(index, listener)| (listener.id, index));
        self.set_selection(selection);
    }

    /// Reconciles the selection with a fresh snapshot: keeps the stable
    /// selection when the listener survived, otherwise falls back to the
    /// first visible listener (or none).
    pub fn refresh_with(&mut self, snapshot: SystemSnapshot) {
        if self.recent.has_baseline() && snapshot.captured_at < self.snapshot.captured_at {
            return;
        }
        if self
            .pending_stop_feedback
            .as_ref()
            .is_some_and(|feedback| snapshot.captured_at >= feedback.after)
            && let Some(feedback) = self.pending_stop_feedback.take()
        {
            let (unobserved, total) = feedback.plan.port_counts(&snapshot);
            let ports = match (unobserved, total) {
                (1, 1) => format!(
                    "{} :{} has no visible listener",
                    feedback.plan.target.protocol, feedback.plan.target.port
                ),
                (0, 1) => format!(
                    "{} :{} still occupied",
                    feedback.plan.target.protocol, feedback.plan.target.port
                ),
                _ => format!("{unobserved}/{total} ports have no visible listener"),
            };
            self.notification = Some(Notification::new(format!("{} · {ports}", feedback.outcome)));
        }
        if self
            .stop_plan
            .as_ref()
            .is_some_and(|plan| !plan.matches(&snapshot))
        {
            self.cancel_stop("Stop canceled · target or affected scope changed");
        }
        self.recent.observe(&snapshot);
        self.snapshot = snapshot;
        self.reconcile_selection();
    }

    /// Whether the recently opened view is active.
    pub const fn recent_only(&self) -> bool {
        self.recent_only
    }

    /// How many listeners opened after the initial scan and are still present.
    pub fn recent_count(&self) -> usize {
        self.recent.len()
    }

    /// Whether sockets without an attributed PID are hidden.
    pub const fn known_owners_only(&self) -> bool {
        self.known_owners_only
    }

    /// How many sockets in the active view have no attributed PID.
    pub fn unknown_owner_count(&self) -> usize {
        self.snapshot
            .listeners
            .iter()
            .filter(|listener| {
                listener.pid().as_u32() == 0
                    && (!self.recent_only || self.recent.contains(listener.id))
            })
            .count()
    }

    /// The listeners visible with the current sort and filter applied.
    pub fn visible_listeners(&self) -> Vec<&portspy_core::domain::listener::Listener> {
        let mut filtered =
            portspy_core::snapshot::query::search(&self.snapshot.listeners, &self.query);
        if self.recent_only {
            filtered.retain(|listener| self.recent.contains(listener.id));
        }
        if self.known_owners_only {
            filtered.retain(|listener| listener.pid().as_u32() != 0);
        }
        portspy_core::snapshot::query::sort_listener_refs(&filtered, self.sort, self.order)
    }

    /// The currently selected listener, if any.
    pub fn selected_listener(&self) -> Option<&portspy_core::domain::listener::Listener> {
        let id = self.selected?;
        self.snapshot.listeners.iter().find(|l| l.id == id)
    }

    /// Drops an expired notification.
    pub fn expire_notification(&mut self, lifetime: Duration) {
        if self
            .notification
            .as_ref()
            .is_some_and(|n| n.is_expired(lifetime))
        {
            self.notification = None;
        }
    }
}

#[derive(Debug, Clone)]
struct StopFeedback {
    plan: StopPlan,
    outcome: String,
    after: Instant,
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
    use portspy_core::domain::origin::{OriginInfo, OriginKind};
    use portspy_core::domain::process::{ProcessId, ProcessInfo};
    use portspy_core::domain::protocol::Protocol;
    use portspy_core::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
    use portspy_core::snapshot::SystemSnapshot;
    use portspy_core::snapshot::builder::SnapshotBuilder;
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::time::{Duration, Instant, UNIX_EPOCH};

    struct FakePlatform(Vec<RawListener>);

    impl Platform for FakePlatform {
        fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
            Ok(self.0.clone())
        }

        fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
            Ok(Vec::new())
        }

        fn kill(&self, _: ProcessId) -> Result<KillOutcome, PlatformError> {
            Err(PlatformError::Unsupported("test stop"))
        }
    }

    fn observe(state: &mut AppState, listeners: &[RawListener]) {
        let platform = FakePlatform(listeners.to_vec());
        let snapshot = SnapshotBuilder::new(&platform)
            .snapshot()
            .expect("test scan");
        state.refresh_with(snapshot);
    }

    fn raw_listener(socket: &TcpListener, pid: u32) -> std::io::Result<RawListener> {
        let address = socket.local_addr()?;
        Ok(RawListener {
            protocol: Protocol::Tcp,
            address: address.ip(),
            port: address.port(),
            pid: ProcessId(pid),
        })
    }

    fn listener(port: u16, pid: u32) -> Listener {
        let started_at = Some(UNIX_EPOCH + Duration::from_secs(pid.into()));
        let process = ProcessInfo {
            pid: ProcessId(pid),
            parent_pid: None,
            name: "node.exe".to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at,
            memory_bytes: Some(1024),
            cpu_percent: None,
            ancestors: Vec::new(),
        };
        Listener {
            id: ListenerId::new(
                Protocol::Tcp,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                port,
                ProcessId(pid),
                started_at,
            ),
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
            project: portspy_core::domain::project::ProjectInfo::default(),
            is_duplicate: false,
            is_orphan: false,
        }
    }

    fn state_with(ports: &[u16]) -> AppState {
        let listeners = ports
            .iter()
            .map(|p| listener(*p, *p as u32 + 100))
            .collect();
        let mut state = AppState::new();
        state.snapshot = SystemSnapshot {
            captured_at: std::time::Instant::now(),
            wall_time: std::time::SystemTime::now(),
            listeners,
            processes: Vec::new(),
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
            processes: Vec::new(),
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
    fn owner_filter_keeps_identified_selection_and_can_restore_hidden_sockets()
    -> std::io::Result<()> {
        let identified = TcpListener::bind("127.0.0.1:0")?;
        let hidden = TcpListener::bind("127.0.0.1:0")?;
        let identified = raw_listener(&identified, 20)?;
        let hidden = raw_listener(&hidden, 0)?;
        let mut state = AppState::new();
        observe(&mut state, &[identified.clone(), hidden]);
        assert_eq!(state.visible_listeners().len(), 2);
        assert_eq!(state.unknown_owner_count(), 1);

        state.apply(Action::ToggleKnownOwners);
        assert!(state.known_owners_only());
        assert_eq!(state.visible_listeners().len(), 1);
        assert_eq!(state.selected.map(|id| id.port), Some(identified.port));
        state.apply(Action::ToggleKnownOwners);
        assert_eq!(state.visible_listeners().len(), 2);
        assert_eq!(state.selected.map(|id| id.port), Some(identified.port));
        Ok(())
    }

    #[test]
    fn recent_view_combines_with_search_and_keeps_selection_on_return() -> std::io::Result<()> {
        let baseline = TcpListener::bind("127.0.0.1:0")?;
        let opened = TcpListener::bind("127.0.0.1:0")?;
        let baseline = raw_listener(&baseline, 20)?;
        let opened = raw_listener(&opened, 21)?;
        let mut state = AppState::new();

        observe(&mut state, std::slice::from_ref(&baseline));
        state.apply(Action::ToggleRecent);
        assert!(state.recent_only());
        assert!(state.visible_listeners().is_empty());
        assert!(state.selected.is_none());

        observe(&mut state, &[baseline, opened.clone()]);
        assert_eq!(state.recent_count(), 1);
        assert_eq!(state.selected.map(|id| id.port), Some(opened.port));
        state.apply(Action::Search);
        state.push_search_char('~');
        assert!(state.selected.is_none());
        state.apply(Action::Inspect);
        assert_eq!(state.mode, super::AppMode::Search);
        state.pop_search_char();
        for character in opened.port.to_string().chars() {
            state.push_search_char(character);
        }
        state.apply(Action::Inspect);
        assert_eq!(state.mode, super::AppMode::Inspect);
        state.apply(Action::ScrollDetailsDown);
        assert!(state.inspection_scroll > 0);
        state.apply(Action::Back);
        assert_eq!(state.selected.map(|id| id.port), Some(opened.port));
        assert_eq!(state.query, opened.port.to_string());
        state.apply(Action::Back);
        state.apply(Action::ToggleRecent);
        assert!(!state.recent_only());
        assert_eq!(state.visible_listeners().len(), 2);
        assert_eq!(state.selected.map(|id| id.port), Some(opened.port));
        state.apply(Action::Inspect);
        assert_eq!(state.inspection_scroll, 0);
        Ok(())
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
        let plan = state.stop_plan.as_ref().expect("review");
        assert_eq!(Some(plan.target), state.selected);
        assert!(!plan.tree);
        state.apply(Action::Cancel);
        assert_eq!(state.mode, super::AppMode::Browse);
        assert!(state.stop_plan.is_none());
    }

    #[test]
    fn tree_kill_confirmation_round_trip() {
        let mut state = state_with(&[3000]);
        state.apply(Action::MoveDown);
        state.apply(Action::RequestKillTree);
        assert_eq!(state.mode, super::AppMode::ConfirmKill);
        let plan = state.stop_plan.as_ref().expect("review");
        assert_eq!(Some(plan.target), state.selected);
        assert!(plan.tree);
        state.apply(Action::ConfirmKill);
        assert_eq!(state.mode, super::AppMode::Browse);
        assert!(state.stop_plan.is_none());
    }

    #[test]
    fn tree_kill_requires_selection() {
        let mut state = state_with(&[3000]);
        state.apply(Action::RequestKillTree);
        assert_eq!(state.mode, super::AppMode::Browse);
        assert!(state.stop_plan.is_none());
    }

    #[test]
    fn stop_review_includes_all_listener_ports_and_cancels_when_scope_changes()
    -> std::io::Result<()> {
        let first = TcpListener::bind("127.0.0.1:0")?;
        let second = TcpListener::bind("127.0.0.1:0")?;
        let port = first.local_addr()?.port();
        let other_port = second.local_addr()?.port();
        let mut state = state_with(&[port]);
        let owner = state.snapshot.listeners[0].pid().as_u32();
        state.snapshot.listeners.push(listener(other_port, owner));
        state.apply(Action::MoveDown);
        state.apply(Action::Inspect);
        state.apply(Action::RequestKill);
        assert_eq!(state.stop_plan.as_ref().expect("review").listeners.len(), 2);

        let mut fresh = state.snapshot.clone();
        fresh
            .listeners
            .retain(|listener| listener.port != other_port);
        state.refresh_with(fresh);
        assert_eq!(state.mode, super::AppMode::Inspect);
        assert!(state.stop_plan.is_none());
        assert!(
            state
                .notification
                .expect("notification")
                .message
                .contains("scope changed")
        );
        Ok(())
    }

    #[test]
    fn only_a_post_stop_scan_can_report_that_a_listener_is_no_longer_visible() -> std::io::Result<()>
    {
        use portspy_core::processes::tree::TreeKillReport;

        let socket = TcpListener::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        let mut state = state_with(&[port]);
        state.apply(Action::MoveDown);
        state.apply(Action::RequestKill);
        let plan = state.stop_plan.clone().expect("review");
        let report = TreeKillReport {
            terminated: vec![plan.process.identity.pid],
            ..Default::default()
        };
        state.apply(Action::ConfirmKill);
        state.report_stop(plan, report);

        let mut fresh = state.snapshot.clone();
        fresh.listeners.clear();
        state.refresh_with(fresh.clone());
        assert!(
            state
                .notification
                .as_ref()
                .expect("notification")
                .message
                .contains("checking ports")
        );
        fresh.captured_at = Instant::now();
        state.refresh_with(fresh);
        assert!(
            state
                .notification
                .expect("notification")
                .message
                .contains("has no visible listener")
        );
        Ok(())
    }

    #[test]
    fn stop_feedback_counts_only_ports_with_no_visible_listener() -> std::io::Result<()> {
        use portspy_core::processes::tree::TreeKillReport;

        let first = TcpListener::bind("127.0.0.1:0")?;
        let second = TcpListener::bind("127.0.0.1:0")?;
        let port = first.local_addr()?.port();
        let other_port = second.local_addr()?.port();
        let mut state = state_with(&[port]);
        let owner = state.snapshot.listeners[0].pid().as_u32();
        state.snapshot.listeners.push(listener(other_port, owner));
        state.apply(Action::MoveDown);
        state.apply(Action::RequestKill);
        let plan = state.begin_stop().expect("review");
        let report = TreeKillReport {
            terminated: vec![plan.process.identity.pid],
            ..Default::default()
        };
        state.report_stop(plan, report);

        let mut fresh = state.snapshot.clone();
        fresh.listeners = vec![listener(other_port, owner + 1)];
        fresh.captured_at = Instant::now();
        state.refresh_with(fresh);
        assert!(
            state
                .notification
                .expect("notification")
                .message
                .contains("1/2 ports have no visible listener")
        );
        Ok(())
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
