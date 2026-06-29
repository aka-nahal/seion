//! A centered overlay box — clears what's behind it, then draws a soft panel.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Clear;

use crate::theme::Theme;
use crate::ui;

/// Draw a centered `width`×`height` titled panel, clearing what's behind it,
/// and return the inner content [`Rect`]. Callers lay out their own body (a
/// paragraph, or the multi-column help) inside this consistent overlay frame.
pub fn panel_box(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    width: u16,
    height: u16,
) -> Rect {
    let rect = ui::centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = ui::panel(theme, title);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    inner
}
