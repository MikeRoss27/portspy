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
    ScrollDetailsUp,
    ScrollDetailsDown,
    Inspect,
    Back,
    Search,
    ClearSearch,
    ToggleRecent,
    ToggleKnownOwners,
    SortNextField,
    ToggleSortOrder,
    RequestKill,
    RequestKillTree,
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
    Action::ScrollDetailsUp,
    Action::ScrollDetailsDown,
    Action::Inspect,
    Action::Back,
    Action::Search,
    Action::ClearSearch,
    Action::ToggleRecent,
    Action::ToggleKnownOwners,
    Action::SortNextField,
    Action::ToggleSortOrder,
    Action::RequestKill,
    Action::RequestKillTree,
    Action::ConfirmKill,
    Action::Cancel,
];
