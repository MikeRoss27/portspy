//! Browse screen: header, listeners table, footer.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::state::{AppMode, AppState};
use crate::ui::components::history::Trend;
use crate::ui::components::{details_panel, footer, header, listeners_table, search};
use crate::ui::layout::{browse_layout, main_layout};

/// Renders the browse screen.
pub fn render(frame: &mut Frame<'_>, state: &mut AppState) {
    let (header_area, body_area, footer_area) = main_layout(frame.area());
    header::render(frame, header_area, state);
    let (list_area, preview_area) = browse_layout(body_area, state.preview_enabled());
    let table_area = if state.mode == AppMode::Search {
        let [prompt, table] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(list_area);
        search::render(frame, prompt, &state.query);
        table
    } else {
        list_area
    };
    let now = Instant::now();
    let listeners = Trend::listeners(state, now);
    let (table_area, listener_history) = listeners.layout(table_area);
    listeners_table::render(frame, table_area, state);
    if let Some(trend) = listener_history {
        listeners.render(frame, trend);
    }
    if let Some(preview_area) = preview_area {
        let memory = Trend::memory(state, now);
        let (preview_area, process_history) = if preview_area.height >= 20 {
            memory.layout(preview_area)
        } else {
            (preview_area, None)
        };
        details_panel::preview(frame, preview_area, state);
        if let Some(trend) = process_history {
            memory.render(frame, trend);
        }
    }
    footer::render(frame, footer_area, state);
}
