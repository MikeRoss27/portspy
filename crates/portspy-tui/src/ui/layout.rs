//! Responsive layout.
//!
//! The layout adapts to the terminal width:
//!
//! ```text
//! wide    PORT  APPLICATION  ORIGIN  PID  MEMORY  UPTIME
//! medium  PORT  APPLICATION  ORIGIN  MEMORY  UPTIME
//! small   PORT  APPLICATION  ORIGIN  MEMORY  UPTIME  (no PID)
//! ```

use ratatui::layout::{Constraint, Layout, Rect};

/// Which column set fits the current width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    Narrow,
    Medium,
    Wide,
}

impl Breakpoint {
    /// Picks the breakpoint for a width.
    pub fn of(width: u16) -> Self {
        if width >= 96 {
            Self::Wide
        } else if width >= 72 {
            Self::Medium
        } else {
            Self::Narrow
        }
    }
}

/// Splits the screen into header, body and footer areas.
pub fn main_layout(area: Rect) -> (Rect, Rect, Rect) {
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let mut chunks = vertical.split(area);
    let header = chunks[0];
    let body = chunks[1];
    let footer = chunks[2];
    let _ = &mut chunks;
    (header, body, footer)
}

/// Splits the inspect screen into the listener list and the details panel.
pub fn inspect_layout(area: Rect) -> (Rect, Rect) {
    let horizontal = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]);
    let chunks = horizontal.split(area);
    (chunks[0], chunks[1])
}

#[cfg(test)]
mod tests {
    use super::{main_layout, Breakpoint};
    use ratatui::layout::Rect;

    #[test]
    fn breakpoints_track_width() {
        assert_eq!(Breakpoint::of(40), Breakpoint::Narrow);
        assert_eq!(Breakpoint::of(71), Breakpoint::Narrow);
        assert_eq!(Breakpoint::of(72), Breakpoint::Medium);
        assert_eq!(Breakpoint::of(95), Breakpoint::Medium);
        assert_eq!(Breakpoint::of(96), Breakpoint::Wide);
        assert_eq!(Breakpoint::of(200), Breakpoint::Wide);
    }

    #[test]
    fn main_layout_orders_areas() {
        let (header, body, footer) = main_layout(Rect::new(0, 0, 100, 30));
        assert!(header.y < body.y);
        assert!(body.y < footer.y);
        assert_eq!(header.height, 3);
        assert_eq!(footer.height, 1);
    }
}
