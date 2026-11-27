//! Responsive layout.
//!
//! The layout adapts to the terminal width:
//!
//! ```text
//! full    PORT  APP  PROJECT  ORIGIN  BINDING  PID  PROCESS  MEMORY  AGE
//! wide    PORT  APP  PROJECT  ORIGIN  BINDING  MEMORY  AGE
//! medium  PORT  APP  PROJECT  ORIGIN  BINDING
//! narrow  PORT / BINDING      APP / PROJECT / ORIGIN
//! ```

use ratatui::layout::{Constraint, Layout, Rect};

/// Which column set fits the current width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    Narrow,
    Medium,
    Wide,
    Full,
}

impl Breakpoint {
    /// Picks the breakpoint for a width.
    pub fn of(width: u16) -> Self {
        if width >= 124 {
            Self::Full
        } else if width >= 96 {
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
        Constraint::Length(if area.height < 16 { 2 } else { 3 }),
        Constraint::Min(0),
        Constraint::Length(if area.width < 130 && area.height >= 20 {
            2
        } else {
            1
        }),
    ]);
    let chunks = vertical.split(area);
    let header = chunks[0];
    let body = chunks[1];
    let footer = chunks[2];
    (header, body, footer)
}

/// Reserves room for a preview while keeping the main listener columns readable.
pub(crate) fn browse_layout(area: Rect, preview: bool) -> (Rect, Option<Rect>) {
    if !preview || area.width < 116 || area.height < 12 {
        return (area, None);
    }
    let preview_width = (area.width / 3).clamp(38, 52);
    let [list, _, preview] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(preview_width),
    ])
    .areas(area);
    (list, Some(preview))
}

/// Shows the listener list alongside details when the terminal is wide enough.
pub fn inspect_layout(area: Rect) -> (Option<Rect>, Rect) {
    if !matches!(
        Breakpoint::of(area.width),
        Breakpoint::Wide | Breakpoint::Full
    ) {
        return (None, area);
    }
    let [list, _, inspector] = Layout::horizontal([
        Constraint::Percentage(55),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    (Some(list), inspector)
}

#[cfg(test)]
mod tests {
    use super::{Breakpoint, inspect_layout, main_layout};
    use ratatui::layout::Rect;

    #[test]
    fn breakpoints_track_width() {
        assert_eq!(Breakpoint::of(40), Breakpoint::Narrow);
        assert_eq!(Breakpoint::of(71), Breakpoint::Narrow);
        assert_eq!(Breakpoint::of(72), Breakpoint::Medium);
        assert_eq!(Breakpoint::of(95), Breakpoint::Medium);
        assert_eq!(Breakpoint::of(96), Breakpoint::Wide);
        assert_eq!(Breakpoint::of(123), Breakpoint::Wide);
        assert_eq!(Breakpoint::of(124), Breakpoint::Full);
    }

    #[test]
    fn main_layout_orders_areas() {
        let (header, body, footer) = main_layout(Rect::new(0, 0, 100, 30));
        assert!(header.y < body.y);
        assert!(body.y < footer.y);
        assert_eq!(header.height, 3);
        assert_eq!(footer.height, 2);
        let (header, _, footer) = main_layout(Rect::new(0, 0, 64, 12));
        assert_eq!((header.height, footer.height), (2, 1));
    }

    #[test]
    fn inspector_fills_compact_terminal_and_splits_wide_terminal() {
        let compact = Rect::new(0, 3, 80, 20);
        assert_eq!(inspect_layout(compact), (None, compact));

        let wide = Rect::new(0, 3, 120, 20);
        let (list, details) = inspect_layout(wide);
        assert!(list.is_some_and(|list| list.right() + 1 == details.left()));
        assert_eq!(details.right(), wide.right());
    }
}
