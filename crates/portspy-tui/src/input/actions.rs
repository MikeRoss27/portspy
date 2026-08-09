//! Application actions: the vocabulary the UI understands.
//!
//! Key presses are translated to actions by [`super::keymap`]; the app
//! reacts to actions, never to raw keys.

/// Every user-intent in the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    MoveUp,
    MoveDown,
    MoveTop,
    MoveBottom,
    Inspect,
    Back,
    Search,
    ClearSearch,
    SortNextField,
    ToggleSortOrder,
    RequestKill,
    ConfirmKill,
    Cancel,
}

/// The complete set of actions each mode accepts.
pub const ALL_ACTIONS: &[Action] = &[
    Action::Quit,
    Action::MoveUp,
    Action::MoveDown,
    Action::MoveTop,
    Action::MoveBottom,
    Action::Inspect,
    Action::Back,
    Action::Search,
    Action::ClearSearch,
    Action::SortNextField,
    Action::ToggleSortOrder,
    Action::RequestKill,
    Action::ConfirmKill,
    Action::Cancel,
];
