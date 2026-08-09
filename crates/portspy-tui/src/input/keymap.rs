//! Key-to-action translation.
//!
//! The UI never matches raw keys; it matches [`Action`]s produced here.
//! Custom keybindings can be added later without touching the app logic.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::actions::Action;
use crate::state::AppMode;

/// Translates a key press into an action for the current mode.
///
/// Returns `None` for keys with no meaning in the current mode.
pub fn map_key(key: KeyEvent, mode: AppMode) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        return Some(Action::Quit);
    }

    match mode {
        AppMode::Browse | AppMode::Inspect => map_browse_key(key),
        AppMode::Search => map_search_key(key),
        AppMode::ConfirmKill => map_confirm_key(key),
    }
}

fn map_browse_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char('g') => Some(Action::MoveTop),
        KeyCode::Char('G') | KeyCode::End => Some(Action::MoveBottom),
        KeyCode::Enter => Some(Action::Inspect),
        KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('h') | KeyCode::Left => {
            Some(Action::Back)
        }
        KeyCode::Char('/') => Some(Action::Search),
        KeyCode::Char('s') => Some(Action::SortNextField),
        KeyCode::Char('S') => Some(Action::ToggleSortOrder),
        // `k` stays vim-up; killing is `x` (like "close" in mail clients).
        KeyCode::Char('x') => Some(Action::RequestKill),
        _ => None,
    }
}

fn map_search_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Cancel),
        KeyCode::Enter => Some(Action::Back),
        _ => None,
    }
}

fn map_confirm_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Action::ConfirmKill),
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => Some(Action::Cancel),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::map_key;
    use crate::input::actions::Action;
    use crate::state::AppMode;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn key(code: KeyCode) -> KeyEvent {
        press(code, KeyModifiers::NONE)
    }

    #[test]
    fn vim_and_arrow_navigation() {
        assert_eq!(map_key(key(KeyCode::Char('j')), AppMode::Browse), Some(Action::MoveDown));
        assert_eq!(map_key(key(KeyCode::Down), AppMode::Browse), Some(Action::MoveDown));
        assert_eq!(map_key(key(KeyCode::Char('k')), AppMode::Browse), Some(Action::MoveUp));
        assert_eq!(map_key(key(KeyCode::Up), AppMode::Browse), Some(Action::MoveUp));
    }

    #[test]
    fn jump_to_ends() {
        assert_eq!(map_key(key(KeyCode::Char('g')), AppMode::Browse), Some(Action::MoveTop));
        assert_eq!(map_key(key(KeyCode::Char('G')), AppMode::Browse), Some(Action::MoveBottom));
    }

    #[test]
    fn browse_mode_actions() {
        assert_eq!(map_key(key(KeyCode::Enter), AppMode::Browse), Some(Action::Inspect));
        assert_eq!(map_key(key(KeyCode::Char('/')), AppMode::Browse), Some(Action::Search));
        assert_eq!(map_key(key(KeyCode::Char('s')), AppMode::Browse), Some(Action::SortNextField));
        assert_eq!(map_key(key(KeyCode::Char('x')), AppMode::Browse), Some(Action::RequestKill));
        assert_eq!(map_key(key(KeyCode::Char('q')), AppMode::Browse), Some(Action::Quit));
        assert_eq!(map_key(key(KeyCode::Esc), AppMode::Browse), Some(Action::Back));
    }

    #[test]
    fn control_c_always_quits() {
        let ctrl_c = press(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(map_key(ctrl_c, AppMode::Browse), Some(Action::Quit));
        assert_eq!(map_key(ctrl_c, AppMode::Search), Some(Action::Quit));
        assert_eq!(map_key(ctrl_c, AppMode::ConfirmKill), Some(Action::Quit));
    }

    #[test]
    fn search_mode_only_accepts_cancel() {
        assert_eq!(map_key(key(KeyCode::Esc), AppMode::Search), Some(Action::Cancel));
        assert_eq!(map_key(key(KeyCode::Enter), AppMode::Search), Some(Action::Back));
        assert_eq!(map_key(key(KeyCode::Char('j')), AppMode::Search), None);
        assert_eq!(map_key(key(KeyCode::Char('q')), AppMode::Search), None);
    }

    #[test]
    fn confirm_mode_accepts_yes_no_esc() {
        assert_eq!(map_key(key(KeyCode::Char('y')), AppMode::ConfirmKill), Some(Action::ConfirmKill));
        assert_eq!(map_key(key(KeyCode::Enter), AppMode::ConfirmKill), Some(Action::ConfirmKill));
        assert_eq!(map_key(key(KeyCode::Esc), AppMode::ConfirmKill), Some(Action::Cancel));
        assert_eq!(map_key(key(KeyCode::Char('n')), AppMode::ConfirmKill), Some(Action::Cancel));
    }

    #[test]
    fn kill_is_never_confused_with_vim_up() {
        assert_eq!(map_key(key(KeyCode::Char('k')), AppMode::Browse), Some(Action::MoveUp));
        assert_eq!(map_key(key(KeyCode::Char('x')), AppMode::Browse), Some(Action::RequestKill));
    }
}
