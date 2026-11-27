//! Compact history charts for all listeners and the selected process.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::collector::REFRESH_INTERVAL;
use crate::format::memory;
use crate::history::{LISTENER_WINDOW, MEMORY_WINDOW};
use crate::state::{AppState, ScanStatus};
use crate::ui::theme::Theme;

const MAX_SAMPLE_GAP: Duration = REFRESH_INTERVAL.saturating_add(Duration::from_secs(1));

#[derive(Clone, Copy)]
enum Metric {
    Listeners,
    Memory,
}

impl Metric {
    fn label(self) -> &'static str {
        match self {
            Self::Listeners => "All listeners",
            Self::Memory => "Process memory",
        }
    }

    fn format(self, value: u64) -> String {
        match self {
            Self::Listeners => value.to_string(),
            Self::Memory => memory::memory(Some(value)),
        }
    }
}

pub(crate) struct Trend {
    metric: Metric,
    points: Vec<(Duration, Option<u64>)>,
    stale: bool,
}

impl Trend {
    pub(crate) fn listeners(state: &AppState, now: Instant) -> Self {
        Self {
            metric: Metric::Listeners,
            points: state.history.listeners(now),
            stale: state.scan_status != ScanStatus::Current,
        }
    }

    pub(crate) fn memory(state: &AppState, now: Instant) -> Self {
        Self {
            metric: Metric::Memory,
            points: state.selected_listener().map_or_else(Vec::new, |listener| {
                state.history.memory(now, listener.process.identity())
            }),
            stale: state.scan_status != ScanStatus::Current,
        }
    }

    pub(crate) fn layout(&self, area: Rect) -> (Rect, Option<Rect>) {
        let samples = self
            .points
            .iter()
            .filter(|(_, value)| value.is_some())
            .count();
        if samples == 0 || area.width < 34 || area.height < 16 {
            return (area, None);
        }

        let height = if samples == 1 {
            2
        } else {
            (area.height / 4).clamp(5, 10)
        };
        let [main, _, trend] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(height),
        ])
        .areas(area);
        (main, Some(trend))
    }

    pub(crate) fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let mut values = self
            .points
            .iter()
            .filter_map(|&(age, value)| Some((age, value?)));
        let Some((oldest, first)) = values.next() else {
            return;
        };
        let (min, max, latest) = values.fold((first, first, first), |(min, max, _), (_, value)| {
            (min.min(value), max.max(value), value)
        });
        let theme = Theme::default();
        let last = self.stale || self.points.last().is_some_and(|(_, value)| value.is_none());
        let status = if last { "  Last " } else { "  " };
        let panel = theme.panel("~", self.metric.label()).title_top(
            Line::from(Span::styled(
                format!("{status}{} ", self.metric.format(latest)),
                theme.key(),
            ))
            .right_aligned(),
        );
        let inner = panel.inner(area);
        frame.render_widget(panel, area);
        if inner.height < 4 {
            frame.render_widget(
                Paragraph::new("Collecting history…").style(theme.muted_text()),
                inner,
            );
            return;
        }

        let window = history_window(self.metric, oldest);
        let (low, high) = (self.metric.format(min), self.metric.format(max));
        // The general memory formatter rounds values to whole MB.
        let (low, high) = if min != max && low == high {
            (format!("{min} B"), format!("{max} B"))
        } else {
            (low, high)
        };
        let axis_width = (low.len().max(high.len()) as u16 + 2).min(inner.width / 2);
        let [axis, plot] =
            Layout::horizontal([Constraint::Length(axis_width), Constraint::Min(0)]).areas(inner);
        let [plot, time] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(plot);

        let mut labels = vec![Line::from("│"); usize::from(plot.height)];
        if min == max {
            labels[usize::from(plot.height / 2)] = Line::from(format!("{high} ┤"));
        } else {
            labels[0] = Line::from(format!("{high} ┤"));
            labels[usize::from(plot.height - 1)] = Line::from(format!("{low} ┤"));
        }
        frame.render_widget(
            Paragraph::new(labels)
                .right_aligned()
                .style(theme.muted_text()),
            axis,
        );
        self.draw_area(frame, plot, window, min, max);

        let start = if window.as_secs() < 60 {
            format!("−{} s", window.as_secs())
        } else {
            format!("−{} min", window.as_secs() / 60)
        };
        let rule_width = usize::from(time.width).saturating_sub(start.chars().count() + 5);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(start),
                Span::raw(format!(" {} ", "─".repeat(rule_width))),
                Span::raw("Now"),
            ]))
            .style(theme.muted_text()),
            time,
        );
    }

    fn draw_area(&self, frame: &mut Frame<'_>, plot: Rect, window: Duration, min: u64, max: u64) {
        // A Braille cell has two dot columns and four dot rows. Preserve peaks
        // when multiple observations land in the same column.
        let width = usize::from(plot.width) * 2;
        let height = u32::from(plot.height) * 4;
        if width == 0 || height == 0 {
            return;
        }
        let mut heights = vec![0; width];
        let mut previous = None;
        for &(age, value) in &self.points {
            let Some(value) = value else {
                previous = None;
                continue;
            };
            let x = ((1.0 - age.as_secs_f64() / window.as_secs_f64()).clamp(0.0, 1.0)
                * (width - 1) as f64)
                .round() as usize;
            let level = if min == max {
                height.div_ceil(2)
            } else {
                1 + ((u128::from(value - min) * u128::from(height - 1)) / u128::from(max - min))
                    as u32
            };
            if let Some((age_before, start, last)) = previous
                && age_before - age <= MAX_SAMPLE_GAP
            {
                for dot in &mut heights[start..x] {
                    *dot = (*dot).max(last);
                }
            }
            heights[x] = heights[x].max(level);
            previous = Some((age, x, level));
        }

        let colors = match self.metric {
            Metric::Listeners => [(65, 161, 129), (159, 214, 119), (250, 191, 106)],
            Metric::Memory => [(78, 134, 191), (152, 150, 235), (228, 151, 213)],
        };
        // Unicode Braille bit masks, indexed by the number of filled dots
        // from the bottom of each half-cell.
        const LEFT: [u8; 5] = [0, 64, 68, 70, 71];
        const RIGHT: [u8; 5] = [0, 128, 160, 176, 184];
        for row in 0..plot.height {
            let base = u32::from(plot.height - 1 - row) * 4;
            let t = f64::from(base) / f64::from(height.saturating_sub(4).max(1)) * 2.0;
            let index = (t as usize).min(1);
            let mix = |a: u8, b: u8| {
                (f64::from(a) + (f64::from(b) - f64::from(a)) * (t - index as f64)).round() as u8
            };
            let (a, b) = (colors[index], colors[index + 1]);
            let color = Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2));
            for (x, dots) in heights.as_chunks::<2>().0.iter().enumerate() {
                let bits = LEFT[dots[0].saturating_sub(base).min(4) as usize]
                    | RIGHT[dots[1].saturating_sub(base).min(4) as usize];
                if bits != 0 {
                    let glyph = char::from_u32(0x2800 + u32::from(bits)).unwrap_or('·');
                    frame.buffer_mut()[(plot.x + x as u16, plot.y + row)]
                        .set_char(glyph)
                        .set_fg(color);
                }
            }
        }
    }
}

fn history_window(metric: Metric, oldest: Duration) -> Duration {
    let max_window = match metric {
        Metric::Listeners => LISTENER_WINDOW,
        Metric::Memory => MEMORY_WINDOW,
    };
    [10, 20, 30, 60, 120, 300]
        .into_iter()
        .map(Duration::from_secs)
        .find(|&window| window >= oldest && window <= max_window)
        .unwrap_or(max_window)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn history_uses_shorter_windows_until_enough_time_has_elapsed() {
        assert_eq!(
            history_window(Metric::Listeners, Duration::from_secs(7)),
            Duration::from_secs(10)
        );
        assert_eq!(
            history_window(Metric::Listeners, Duration::from_secs(308)),
            LISTENER_WINDOW
        );
        assert_eq!(
            history_window(Metric::Memory, Duration::from_secs(50)),
            MEMORY_WINDOW
        );
    }

    #[test]
    fn one_measurement_stays_compact_and_missing_memory_leaves_room_for_details() {
        let area = Rect::new(0, 0, 80, 20);
        let mut trend = Trend {
            metric: Metric::Memory,
            points: vec![(Duration::ZERO, None)],
            stale: false,
        };
        assert_eq!(trend.layout(area), (area, None));
        trend.points[0].1 = Some(1024);
        assert!(trend.layout(area).1.is_some_and(|chart| chart.height == 2));
        trend.points.insert(0, (REFRESH_INTERVAL, Some(1024)));
        assert!(trend.layout(area).1.is_some_and(|chart| chart.height == 5));
    }

    #[test]
    fn flat_series_has_a_scale_and_leaves_missing_intervals_blank()
    -> Result<(), std::convert::Infallible> {
        let trend = Trend {
            metric: Metric::Listeners,
            points: vec![
                (Duration::from_secs(10), Some(0)),
                (Duration::from_secs(8), Some(0)),
                (Duration::from_secs(2), Some(0)),
                (Duration::ZERO, Some(0)),
            ],
            stale: true,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 5))?;
        terminal.draw(|frame| trend.render(frame, frame.area()))?;
        let buffer = terminal.backend().buffer();
        let row = |y| (0..40).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        assert!(row(0).contains("All listeners"));
        assert!(row(0).contains("Last 0"));
        assert!(row(2).contains("0 ┤"));
        assert!(row(4).contains("−10 s"));
        assert!(row(4).contains("Now"));
        assert_eq!(buffer[(20, 2)].symbol(), " ");
        assert_eq!(buffer[(5, 3)].symbol(), "⣿");
        assert_ne!(buffer[(37, 2)].fg, buffer[(37, 3)].fg);
        Ok(())
    }
}
