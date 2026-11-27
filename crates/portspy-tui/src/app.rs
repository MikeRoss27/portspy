//! The application: event loop, input translation and snapshot wiring.
//!
//! This is the only place that talks to the terminal runtime (crossterm).
//! Rendering stays pure; state changes happen through
//! [`crate::state::AppState::apply`].

use std::io;
use std::panic;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::{Terminal, backend::CrosstermBackend};

use portspy_core::error::PortSpyError;
use portspy_core::platform::Platform;
use portspy_core::processes::stop::StopPlan;
use portspy_core::processes::tree::TreeKillReport;
use portspy_core::snapshot::SystemSnapshot;
use portspy_core::snapshot::builder::SnapshotBuilder;

use crate::collector;
use crate::input::actions::Action;
use crate::input::keymap::map_key;
use crate::state::{AppMode, AppState};
use crate::ui;

/// Messages sent from the background threads to the UI loop.
#[derive(Debug)]
pub enum Message {
    Snapshot(SystemSnapshot),
    CollectionError(PortSpyError),
    StopCanceled(String),
    StopFinished {
        plan: StopPlan,
        report: TreeKillReport,
    },
}

impl Message {
    fn apply(self, state: &mut AppState) {
        match self {
            Self::Snapshot(snapshot) => state.refresh_with(snapshot),
            Self::CollectionError(error) => state.report_scan_failure(error),
            Self::StopCanceled(reason) => state.stop_validation_failed(reason),
            Self::StopFinished { plan, report } => state.report_stop(plan, report),
        }
    }
}

/// How long a notification stays visible.
const NOTIFICATION_LIFETIME: Duration = Duration::from_secs(4);

/// Max time the event loop blocks waiting for a key, so snapshot redraws
/// stay fluid.
const KEY_POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// The interactive session.
pub struct App {
    platform: Arc<dyn Platform + Sync>,
}

impl App {
    /// Creates the app around the injected platform.
    pub fn new(platform: Arc<dyn Platform + Sync>) -> Self {
        Self { platform }
    }

    /// Runs the interface until the user quits.
    ///
    /// # Errors
    ///
    /// Returns when the terminal cannot be switched to raw mode, or when the
    /// backing terminal errors during the session.
    pub fn run(&self) -> Result<(), io::Error> {
        // Install panic hook to restore terminal on panic
        let original_hook = panic::take_hook();
        let restore_terminal = || {
            let _ = terminal::disable_raw_mode();
            let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        };
        panic::set_hook(Box::new(move |info| {
            restore_terminal();
            original_hook(info);
        }));

        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = self.session(&mut terminal);

        // Restore terminal on normal exit
        restore_terminal();

        // Note: original_hook is moved into the panic hook closure above.
        // We don't restore it here since the process is about to exit.
        // The panic hook will call the original hook if a panic occurs.

        result
    }

    fn session(
        &self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<(), io::Error> {
        let mut state = AppState::new();
        let (tx, rx) = mpsc::channel();
        let _collector = collector::spawn(self.platform.clone(), tx.clone())?;
        let session_alive = Arc::new(());
        self.draw(terminal, &mut state)?;

        loop {
            // Apply everything the workers produced since the last frame.
            while let Ok(message) = rx.try_recv() {
                message.apply(&mut state);
            }
            state.expire_notification(NOTIFICATION_LIFETIME);
            self.draw(terminal, &mut state)?;

            if !event::poll(KEY_POLL_TIMEOUT)? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if self.handle_key(&mut state, key, terminal, &tx, &session_alive)? {
                return Ok(());
            }
        }
    }

    fn handle_key(
        &self,
        state: &mut AppState,
        key: KeyEvent,
        terminal: &Terminal<CrosstermBackend<io::Stdout>>,
        tx: &mpsc::Sender<Message>,
        session_alive: &Arc<()>,
    ) -> Result<bool, io::Error> {
        let action = map_key(key, state.mode);
        if action == Some(Action::Quit) {
            return Ok(true);
        }
        if state.mode == AppMode::Search {
            match key.code {
                crossterm::event::KeyCode::Char(character) => {
                    state.push_search_char(character);
                }
                crossterm::event::KeyCode::Backspace => {
                    state.pop_search_char();
                }
                _ => {
                    // Only handle explicitly mapped keys in search mode; ignore others
                    if let Some(action) = action {
                        state.apply(action);
                    }
                }
            }
            return Ok(false);
        }

        match action {
            Some(Action::ConfirmKill) if state.mode == AppMode::ConfirmKill => {
                let size = terminal.size()?;
                if ui::components::confirm::can_review(size.width, size.height) {
                    self.confirm_stop(state, tx, session_alive);
                } else {
                    state.cancel_stop("Stop canceled · terminal too small to review");
                }
                Ok(false)
            }
            Some(action) => {
                state.apply(action);
                Ok(false)
            }
            None => Ok(false),
        }
    }

    fn confirm_stop(
        &self,
        state: &mut AppState,
        tx: &mpsc::Sender<Message>,
        session_alive: &Arc<()>,
    ) {
        let Some(plan) = state.begin_stop() else {
            return;
        };
        let platform = self.platform.clone();
        let tx = tx.clone();
        let session_alive = Arc::downgrade(session_alive);
        let worker = thread::Builder::new()
            .name("portspy-stop".to_owned())
            .spawn(move || {
                let fresh = SnapshotBuilder::new(platform.as_ref()).snapshot();
                let message = match fresh {
                    Err(error) => {
                        Message::StopCanceled(format!("Stop canceled · fresh scan failed: {error}"))
                    }
                    Ok(snapshot) if !plan.matches(&snapshot) => Message::StopCanceled(
                        "Stop canceled · target or affected scope changed".into(),
                    ),
                    Ok(_) => {
                        // The session can close while the scan waits on the OS.
                        let Some(_session) = session_alive.upgrade() else {
                            return;
                        };
                        let report = plan.execute(platform.as_ref());
                        Message::StopFinished { plan, report }
                    }
                };
                let _ = tx.send(message);
            });
        if let Err(error) = worker {
            state
                .stop_validation_failed(format!("Stop canceled · could not start worker: {error}"));
        }
    }

    fn draw(
        &self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        state: &mut AppState,
    ) -> Result<(), io::Error> {
        terminal.draw(|frame| ui::render(frame, state))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{App, AppMode, AppState, SnapshotBuilder};
    use crate::input::actions::Action;
    use portspy_core::domain::process::{ProcessId, ProcessIdentity};
    use portspy_core::domain::protocol::Protocol;
    use portspy_core::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
    use std::io;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, UNIX_EPOCH};

    const WORKER_TIMEOUT: Duration = Duration::from_secs(2);

    struct ScanPause {
        started: mpsc::Sender<()>,
        resume: Mutex<mpsc::Receiver<()>>,
    }

    #[derive(Default)]
    struct FakePlatform {
        listeners: Vec<RawListener>,
        processes: Vec<RawProcess>,
        stopped: Mutex<Vec<ProcessIdentity>>,
        pause: Option<ScanPause>,
    }

    impl Platform for FakePlatform {
        fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
            if let Some(pause) = &self.pause {
                pause.started.send(()).expect("scan observer");
                pause
                    .resume
                    .lock()
                    .expect("scan pause")
                    .recv_timeout(WORKER_TIMEOUT)
                    .expect("resume scan");
            }
            Ok(self.listeners.clone())
        }

        fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
            Ok(self.processes.clone())
        }

        fn kill(&self, _: ProcessId) -> Result<KillOutcome, PlatformError> {
            Err(PlatformError::Unsupported("unchecked stop"))
        }

        fn kill_checked(&self, identity: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
            self.stopped.lock().expect("stop record").push(identity);
            Ok(KillOutcome::Terminated)
        }
    }

    fn process(pid: u32, parent: Option<u32>) -> RawProcess {
        RawProcess {
            pid: ProcessId(pid),
            parent_pid: parent.map(ProcessId),
            name: format!("process-{pid}"),
            started_at: Some(UNIX_EPOCH + Duration::from_secs(pid.into())),
            executable: None,
            command_line: None,
            working_directory: None,
            memory_bytes: None,
            cpu_percent: None,
        }
    }

    #[test]
    fn stop_is_canceled_when_the_fresh_scan_finds_an_unreviewed_descendant() -> io::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        let listener = RawListener {
            protocol: Protocol::Tcp,
            address: address.ip(),
            port: address.port(),
            pid: ProcessId(71),
            socket_attribution: None,
        };
        let reviewed = FakePlatform {
            listeners: vec![listener.clone()],
            processes: vec![process(71, None), process(72, Some(71))],
            ..Default::default()
        };
        let mut state = AppState::new();
        state.refresh_with(
            SnapshotBuilder::new(&reviewed)
                .snapshot()
                .expect("initial scan"),
        );
        state.apply(Action::RequestKillTree);
        assert_eq!(
            state.stop_plan.as_ref().expect("review").descendants.len(),
            1
        );

        let platform = Arc::new(FakePlatform {
            listeners: vec![listener],
            processes: vec![
                process(71, None),
                process(72, Some(71)),
                process(73, Some(71)),
            ],
            ..Default::default()
        });
        let app = App::new(platform.clone());
        let (tx, rx) = mpsc::channel();
        let session_alive = Arc::new(());
        app.confirm_stop(&mut state, &tx, &session_alive);
        rx.recv_timeout(WORKER_TIMEOUT)
            .expect("worker result")
            .apply(&mut state);

        assert!(platform.stopped.lock().expect("stop record").is_empty());
        assert!(state.stop_plan.is_none());
        assert!(!state.stop_in_progress());
        assert!(
            state
                .notification
                .expect("cancel reason")
                .message
                .contains("scope changed")
        );
        Ok(())
    }

    struct PausedReview {
        state: AppState,
        platform: Arc<FakePlatform>,
        scan_started: mpsc::Receiver<()>,
        resume: mpsc::Sender<()>,
    }

    fn review_and_pause_scan() -> io::Result<PausedReview> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        let mut platform = FakePlatform {
            listeners: vec![RawListener {
                protocol: Protocol::Tcp,
                address: address.ip(),
                port: address.port(),
                pid: ProcessId(71),
                socket_attribution: None,
            }],
            processes: vec![process(71, None)],
            ..Default::default()
        };
        let mut state = AppState::new();
        state.refresh_with(
            SnapshotBuilder::new(&platform)
                .snapshot()
                .expect("initial scan"),
        );
        state.apply(Action::RequestKill);
        let (started, scan_started) = mpsc::channel();
        let (resume, paused) = mpsc::channel();
        platform.pause = Some(ScanPause {
            started,
            resume: Mutex::new(paused),
        });
        Ok(PausedReview {
            state,
            platform: Arc::new(platform),
            scan_started,
            resume,
        })
    }

    #[test]
    fn confirmed_stop_allows_navigation_and_blocks_duplicate_stops_during_scan() -> io::Result<()> {
        let PausedReview {
            mut state,
            platform,
            scan_started,
            resume,
        } = review_and_pause_scan()?;
        let identity = state.stop_plan.as_ref().expect("review").process.identity;
        let app = App::new(platform.clone());
        let (tx, rx) = mpsc::channel();
        let session_alive = Arc::new(());

        app.confirm_stop(&mut state, &tx, &session_alive);
        scan_started
            .recv_timeout(WORKER_TIMEOUT)
            .expect("scan started");
        assert!(state.stop_in_progress());
        assert_eq!(state.mode, AppMode::Browse);
        state.apply(Action::RequestKillTree);
        assert!(state.stop_plan.is_none());
        state.apply(Action::Inspect);
        assert_eq!(state.mode, AppMode::Inspect);
        assert!(platform.stopped.lock().expect("stop record").is_empty());

        resume.send(()).expect("resume scan");
        rx.recv_timeout(WORKER_TIMEOUT)
            .expect("worker result")
            .apply(&mut state);
        assert!(!state.stop_in_progress());
        assert_eq!(*platform.stopped.lock().expect("stop record"), [identity]);
        assert_eq!(state.mode, AppMode::Inspect);
        state.apply(Action::RequestKill);
        assert!(state.stop_plan.is_some());
        Ok(())
    }

    #[test]
    fn closing_the_session_during_the_scan_cancels_the_signal() -> io::Result<()> {
        let PausedReview {
            mut state,
            platform,
            scan_started,
            resume,
        } = review_and_pause_scan()?;
        let app = App::new(platform.clone());
        let (tx, rx) = mpsc::channel();
        let session_alive = Arc::new(());

        app.confirm_stop(&mut state, &tx, &session_alive);
        scan_started
            .recv_timeout(WORKER_TIMEOUT)
            .expect("scan started");
        drop(session_alive);
        drop(tx);
        resume.send(()).expect("resume scan");

        assert!(matches!(
            rx.recv_timeout(WORKER_TIMEOUT),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        assert!(platform.stopped.lock().expect("stop record").is_empty());
        Ok(())
    }
}
