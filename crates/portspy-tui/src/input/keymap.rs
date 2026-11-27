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
        AppMode::Inspect if key.code == KeyCode::PageUp => Some(Action::ScrollDetailsUp),
        AppMode::Inspect if key.code == KeyCode::PageDown => Some(Action::ScrollDetailsDown),
        AppMode::Browse if key.code == KeyCode::Tab => Some(Action::TogglePreview),
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
        KeyCode::Char('r') => Some(Action::ToggleRecent),
        KeyCode::Char('u') => Some(Action::ToggleKnownOwners),
        KeyCode::Char('t') => Some(Action::CycleProtocol),
        KeyCode::Char('s') => Some(Action::SortNextField),
        KeyCode::Char('S') => Some(Action::ToggleSortOrder),
        // `k` stays vim-up; killing is `x` (like "close" in mail clients).
        // Uppercase `X` kills the process tree (process + descendants).
        KeyCode::Char('x') => Some(Action::RequestKill),
        KeyCode::Char('X') => Some(Action::RequestKillTree),
        _ => None,
    }
}

fn map_search_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::ClearSearch),
        KeyCode::Enter => Some(Action::Inspect),
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        _ => None,
    }
}

fn map_confirm_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('y' | 'Y') if key.modifiers.difference(KeyModifiers::SHIFT).is_empty() => {
            Some(Action::ConfirmKill)
        }
        KeyCode::Enter | KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            Some(Action::Cancel)
        }
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ScrollDetailsUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ScrollDetailsDown),
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
        assert_eq!(
            map_key(key(KeyCode::Char('j')), AppMode::Browse),
            Some(Action::MoveDown)
        );
        assert_eq!(
            map_key(key(KeyCode::Down), AppMode::Browse),
            Some(Action::MoveDown)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('k')), AppMode::Browse),
            Some(Action::MoveUp)
        );
        assert_eq!(
            map_key(key(KeyCode::Up), AppMode::Browse),
            Some(Action::MoveUp)
        );
    }

    #[test]
    fn jump_to_ends() {
        assert_eq!(
            map_key(key(KeyCode::Char('g')), AppMode::Browse),
            Some(Action::MoveTop)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('G')), AppMode::Browse),
            Some(Action::MoveBottom)
        );
    }

    #[test]
    fn browse_mode_actions() {
        assert_eq!(
            map_key(key(KeyCode::Char('t')), AppMode::Browse),
            Some(Action::CycleProtocol)
        );
        assert_eq!(
            map_key(key(KeyCode::Tab), AppMode::Browse),
            Some(Action::TogglePreview)
        );
        assert_eq!(map_key(key(KeyCode::Tab), AppMode::Inspect), None);
        assert_eq!(map_key(key(KeyCode::Char('t')), AppMode::Search), None);
        assert_eq!(
            map_key(key(KeyCode::Enter), AppMode::Browse),
            Some(Action::Inspect)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('/')), AppMode::Browse),
            Some(Action::Search)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('s')), AppMode::Browse),
            Some(Action::SortNextField)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('x')), AppMode::Browse),
            Some(Action::RequestKill)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('q')), AppMode::Browse),
            Some(Action::Quit)
        );
        assert_eq!(
            map_key(key(KeyCode::Esc), AppMode::Browse),
            Some(Action::Back)
        );
    }

    #[test]
    fn recent_view_shortcut_only_applies_to_navigation() {
        for mode in [AppMode::Browse, AppMode::Inspect] {
            assert_eq!(
                map_key(key(KeyCode::Char('r')), mode),
                Some(Action::ToggleRecent)
            );
        }
        for mode in [AppMode::Search, AppMode::ConfirmKill] {
            assert_eq!(map_key(key(KeyCode::Char('r')), mode), None);
        }
    }

    #[test]
    fn owner_filter_shortcut_only_applies_to_navigation() {
        for mode in [AppMode::Browse, AppMode::Inspect] {
            assert_eq!(
                map_key(key(KeyCode::Char('u')), mode),
                Some(Action::ToggleKnownOwners)
            );
        }
        for mode in [AppMode::Search, AppMode::ConfirmKill] {
            assert_eq!(map_key(key(KeyCode::Char('u')), mode), None);
        }
    }

    #[test]
    fn page_keys_scroll_only_in_inspect_mode() {
        for (code, action) in [
            (KeyCode::PageUp, Action::ScrollDetailsUp),
            (KeyCode::PageDown, Action::ScrollDetailsDown),
        ] {
            assert_eq!(map_key(key(code), AppMode::Inspect), Some(action));
            assert_eq!(map_key(key(code), AppMode::Browse), None);
        }
        assert_eq!(
            map_key(key(KeyCode::Up), AppMode::ConfirmKill),
            Some(Action::ScrollDetailsUp)
        );
        assert_eq!(
            map_key(key(KeyCode::Down), AppMode::ConfirmKill),
            Some(Action::ScrollDetailsDown)
        );
    }

    #[test]
    fn control_c_always_quits() {
        let ctrl_c = press(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(map_key(ctrl_c, AppMode::Browse), Some(Action::Quit));
        assert_eq!(map_key(ctrl_c, AppMode::Search), Some(Action::Quit));
        assert_eq!(map_key(ctrl_c, AppMode::ConfirmKill), Some(Action::Quit));
    }

    #[test]
    fn search_mode_supports_selecting_and_inspecting_results() {
        assert_eq!(
            map_key(key(KeyCode::Esc), AppMode::Search),
            Some(Action::ClearSearch)
        );
        assert_eq!(
            map_key(key(KeyCode::Enter), AppMode::Search),
            Some(Action::Inspect)
        );
        assert_eq!(
            map_key(key(KeyCode::Down), AppMode::Search),
            Some(Action::MoveDown)
        );
        assert_eq!(map_key(key(KeyCode::Char('j')), AppMode::Search), None);
        assert_eq!(map_key(key(KeyCode::Char('q')), AppMode::Search), None);
    }

    #[test]
    fn confirm_mode_requires_explicit_yes() {
        assert_eq!(
            map_key(key(KeyCode::Char('y')), AppMode::ConfirmKill),
            Some(Action::ConfirmKill)
        );
        assert_eq!(
            map_key(
                press(KeyCode::Char('Y'), KeyModifiers::SHIFT),
                AppMode::ConfirmKill
            ),
            Some(Action::ConfirmKill)
        );
        for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            assert_eq!(
                map_key(press(KeyCode::Char('y'), modifier), AppMode::ConfirmKill),
                None
            );
        }
        assert_eq!(
            map_key(key(KeyCode::Enter), AppMode::ConfirmKill),
            Some(Action::Cancel)
        );
        assert_eq!(
            map_key(key(KeyCode::Esc), AppMode::ConfirmKill),
            Some(Action::Cancel)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('n')), AppMode::ConfirmKill),
            Some(Action::Cancel)
        );
    }

    #[test]
    fn kill_is_never_confused_with_vim_up() {
        assert_eq!(
            map_key(key(KeyCode::Char('k')), AppMode::Browse),
            Some(Action::MoveUp)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('x')), AppMode::Browse),
            Some(Action::RequestKill)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('X')), AppMode::Browse),
            Some(Action::RequestKillTree)
        );
    }
}
