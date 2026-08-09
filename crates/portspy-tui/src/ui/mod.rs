//! Rendering: screens and components.
//!
//! Every function here is a pure `state → widget` mapping. No business
//! logic, no OS access, no mutation of the application state.

pub mod components;
pub mod layout;
pub mod screens;
pub mod theme;

use ratatui::Frame;

use crate::state::AppState;

/// Renders the current mode into the frame.
pub fn render(frame: &mut Frame<'_>, state: &AppState) {
    match state.mode {
        crate::state::AppMode::Browse => screens::listeners::render(frame, state),
        crate::state::AppMode::Inspect => screens::details::render(frame, state),
        crate::state::AppMode::Search => {
            screens::listeners::render(frame, state);
            components::search::overlay(frame, frame.area(), state);
        }
        crate::state::AppMode::ConfirmKill => {
            screens::listeners::render(frame, state);
            components::confirm::overlay(frame, frame.area(), state);
        }
    }
}
