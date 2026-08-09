//! The application: event loop, input translation and snapshot wiring.
//!
//! This is the only place that talks to the terminal runtime (crossterm).
//! Rendering stays pure; state changes happen through
//! [`crate::state::AppState::apply`].

use std::io;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::{backend::CrosstermBackend, Terminal};

use portspy_core::error::PortSpyError;
use portspy_core::platform::{KillOutcome, Platform};
use portspy_core::snapshot::SystemSnapshot;

use crate::collector;
use crate::input::actions::Action;
use crate::input::keymap::map_key;
use crate::state::{AppMode, AppState, Notification};
use crate::ui;

/// Messages sent from the collector thread to the UI loop.
#[derive(Debug)]
pub enum Message {
    Snapshot(SystemSnapshot),
    CollectionError(PortSpyError),
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
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = self.session(&mut terminal);

        terminal::disable_raw_mode()?;
        crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        result
    }

    fn session(&self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), io::Error> {
        let (tx, rx) = mpsc::channel();
        let _collector = collector::spawn(self.platform.clone(), tx)?;

        let mut state = AppState::new();
        self.draw(terminal, &state)?;

        loop {
            // Apply everything the collector produced since the last frame.
            while let Ok(message) = rx.try_recv() {
                match message {
                    Message::Snapshot(snapshot) => {
                        state.refresh_with(snapshot);
                    }
                    Message::CollectionError(error) => {
                        state.notification = Some(Notification::new(format!(
                            "collection failed: {error}"
                        )));
                    }
                }
            }
            state.expire_notification(NOTIFICATION_LIFETIME);
            self.draw(terminal, &state)?;

            if !event::poll(KEY_POLL_TIMEOUT)? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if self.handle_key(&mut state, key)? {
                return Ok(());
            }
        }
    }

    fn handle_key(&self, state: &mut AppState, key: KeyEvent) -> Result<bool, io::Error> {
        if state.mode == AppMode::Search {
            match key.code {
                crossterm::event::KeyCode::Char(character) => {
                    state.push_search_char(character);
                }
                crossterm::event::KeyCode::Backspace => {
                    state.pop_search_char();
                }
                _ => {
                    state.apply(map_key(key, state.mode).unwrap_or(Action::Back));
                }
            }
            return Ok(false);
        }

        match map_key(key, state.mode) {
            Some(Action::Quit) => Ok(true),
            Some(Action::ConfirmKill) if state.mode == AppMode::ConfirmKill => {
                self.kill(state);
                Ok(false)
            }
            Some(action) => {
                state.apply(action);
                Ok(false)
            }
            None => Ok(false),
        }
    }

    fn kill(&self, state: &mut AppState) {
        let Some(listener) = state.kill_target_listener() else {
            state.apply(Action::Cancel);
            return;
        };
        let name = listener.process.display_name().to_owned();
        let pid = listener.pid();
        let port = listener.port;
        let outcome = self.platform.kill(pid);
        state.apply(Action::ConfirmKill);
        state.notification = Some(Notification::new(match outcome {
            Ok(KillOutcome::Terminated) => {
                format!("killed {name} (pid {pid}) on :{port}")
            }
            Ok(KillOutcome::AlreadyExited) => {
                format!("{name} (pid {pid}) already exited")
            }
            Err(error) => format!("kill failed: {error}"),
        }));
    }

    fn draw(
        &self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        state: &AppState,
    ) -> Result<(), io::Error> {
        terminal.draw(|frame| ui::render(frame, state))?;
        Ok(())
    }
}
