//! Browse screen: header, listeners table, footer.

use ratatui::Frame;

use crate::state::AppState;
use crate::ui::components::{footer, header, listeners_table};
use crate::ui::layout::main_layout;

/// Renders the browse screen.
pub fn render(frame: &mut Frame<'_>, state: &AppState) {
    let (header_area, body_area, footer_area) = main_layout(frame.area());
    header::render(frame, header_area, state);
    listeners_table::render(frame, body_area, state);
    footer::render(frame, footer_area, state);
}
