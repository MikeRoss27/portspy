//! Terminal design system.
//!
//! Low noise, few colors. The palette maps semantic roles (primary, accent,
//! warning, danger, success) to terminal colors; terminals without color
//! support degrade to plain text automatically through ratatui.

use ratatui::style::{Color, Modifier, Style};

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
            primary: Color::Cyan,
            secondary: Color::White,
            muted: Color::DarkGray,
            accent: Color::LightCyan,
            warning: Color::Yellow,
            danger: Color::LightRed,
            success: Color::LightGreen,
        }
    }
}

impl Theme {
    /// Style of the header brand.
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
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::REVERSED)
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
