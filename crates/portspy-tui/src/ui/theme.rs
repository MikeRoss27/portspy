//! Terminal design system.
//!
//! Graphite surfaces, a mint accent, and semantic colors for status and scope.

use portspy_core::domain::protocol::Protocol;
use portspy_core::ports::Exposure;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding};

const CANVAS: Color = Color::Rgb(13, 20, 26);
const PANEL: Color = Color::Rgb(17, 27, 35);
const STRIPE: Color = Color::Rgb(21, 33, 42);
const CONTROL: Color = Color::Rgb(32, 49, 60);
const BORDER: Color = Color::Rgb(54, 76, 89);
const SELECTED: Color = Color::Rgb(31, 78, 75);

/// Semantic palette of the interface.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub accent: Color,
    pub warning: Color,
    pub danger: Color,
    pub success: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Rgb(80, 240, 195),
            secondary: Color::Rgb(232, 241, 245),
            muted: Color::Rgb(167, 187, 198),
            accent: Color::Rgb(116, 228, 205),
            warning: Color::Rgb(250, 200, 129),
            danger: Color::Rgb(255, 156, 156),
            success: Color::Rgb(143, 234, 190),
        }
    }
}

impl Theme {
    pub(crate) const fn brand_mark() -> Style {
        Style::new()
            .fg(Color::Rgb(80, 240, 195))
            .add_modifier(Modifier::BOLD)
    }

    pub(crate) fn canvas(self) -> Style {
        Style::new().bg(CANVAS).fg(self.secondary)
    }

    pub(crate) const fn panel_background(self) -> Color {
        PANEL
    }

    pub(crate) fn panel(self, icon: &'static str, label: impl Into<String>) -> Block<'static> {
        Block::default()
            .borders(Borders::TOP)
            .padding(Padding::horizontal(1))
            .style(Style::new().bg(self.panel_background()).fg(self.secondary))
            .border_style(Style::new().fg(BORDER))
            .title(Line::from(vec![
                Span::styled(format!(" {icon} "), self.accent_text()),
                Span::raw(label.into()),
                Span::raw(" "),
            ]))
            .title_style(self.table_header())
    }

    pub(crate) fn table_row(self, index: usize) -> Style {
        Style::new().bg(if index.is_multiple_of(2) {
            PANEL
        } else {
            STRIPE
        })
    }

    pub(crate) fn controls(self) -> Style {
        Style::new().bg(CONTROL).fg(self.secondary)
    }

    pub(crate) fn keycap(self) -> Style {
        self.controls()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    pub(crate) fn protocol(self, protocol: Protocol) -> Style {
        match protocol {
            Protocol::Tcp => self.accent_text(),
            Protocol::Udp => Style::new().fg(Color::Rgb(192, 181, 255)),
        }
    }

    pub(crate) fn exposure(self, exposure: Exposure) -> (&'static str, Style) {
        match exposure {
            Exposure::Loopback => ("○", self.muted_text()),
            Exposure::Interface => ("◇", self.warning_text()),
            Exposure::AllInterfaces => ("◆", self.warning_text()),
        }
    }

    pub(crate) fn key(self) -> Style {
        self.accent_text().add_modifier(Modifier::BOLD)
    }

    /// Style of prominent panel headings.
    pub const fn header(self) -> Style {
        Style::new().fg(self.primary).add_modifier(Modifier::BOLD)
    }

    /// Style of the header counters.
    pub fn muted_text(self) -> Style {
        Style::new().fg(self.muted)
    }

    /// Style of the selected table row.
    pub fn selected_row(self) -> Style {
        Style::new()
            .bg(SELECTED)
            .fg(self.secondary)
            .add_modifier(Modifier::BOLD)
    }

    /// Style of a highlighted label (application / origin).
    pub fn accent_text(self) -> Style {
        Style::new().fg(self.accent)
    }

    /// Style for destructive confirmations.
    pub fn danger_text(self) -> Style {
        Style::new().fg(self.danger).add_modifier(Modifier::BOLD)
    }

    /// Style for success notifications.
    pub fn success_text(self) -> Style {
        Style::new().fg(self.success)
    }

    /// Style for warnings and errors.
    pub fn warning_text(self) -> Style {
        Style::new().fg(self.warning)
    }

    /// Style of the footer hints.
    pub fn footer_hint(self) -> Style {
        Style::new().fg(self.muted)
    }

    /// Style of the table header.
    pub fn table_header(self) -> Style {
        Style::new().fg(self.secondary).add_modifier(Modifier::BOLD)
    }
}
