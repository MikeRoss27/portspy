//! Browse screen: header, listeners table, footer.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::state::{AppMode, AppState};
use crate::ui::components::{footer, header, listeners_table, search};
use crate::ui::layout::main_layout;

/// Renders the browse screen.
pub fn render(frame: &mut Frame<'_>, state: &mut AppState) {
    let (header_area, body_area, footer_area) = main_layout(frame.area());
    header::render(frame, header_area, state);
    let table_area = if state.mode == AppMode::Search {
        let [prompt, table] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(body_area);
        search::render(frame, prompt, &state.query);
        table
    } else {
        body_area
    };
    listeners_table::render(frame, table_area, state);
    footer::render(frame, footer_area, state);
}
