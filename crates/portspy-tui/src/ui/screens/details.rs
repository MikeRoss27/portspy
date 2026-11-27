//! Inspect screen: full-width details or a split view, depending on available space.

use std::time::Instant;

use ratatui::Frame;

use crate::state::AppState;
use crate::ui::components::history::Trend;
use crate::ui::components::{details_panel, footer, header, listeners_table};
use crate::ui::layout::{inspect_layout, main_layout};

/// Renders the inspect screen.
pub fn render(frame: &mut Frame<'_>, state: &mut AppState) {
    let (header_area, body_area, footer_area) = main_layout(frame.area());
    header::render(frame, header_area, state);
    let (table_area, panel_area) = inspect_layout(body_area);
    if let Some(table_area) = table_area {
        listeners_table::render(frame, table_area, state);
    }
    let memory = Trend::memory(state, Instant::now());
    let (panel_area, process_history) = memory.layout(panel_area);
    details_panel::render(frame, panel_area, state);
    if let Some(trend) = process_history {
        memory.render(frame, trend);
    }
    footer::render(frame, footer_area, state);
}
