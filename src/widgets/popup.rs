//! A centered overlay box — clears what's behind it, then draws a soft panel.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::{Clear, Paragraph};

use crate::theme::Theme;
use crate::ui;

/// Draw a centered `width`×`height` overlay with `title` and `body`.
pub fn overlay(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    body: impl Into<Text<'static>>,
    width: u16,
    height: u16,
) {
    let rect = ui::centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = ui::panel(theme, title);
    frame.render_widget(Paragraph::new(body).block(block), rect);
}
