use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

use crate::models::{Toast, ToastKind};
use crate::services::AppService;

const MAX_WIDTH: u16 = 42;
const HEIGHT: u16 = 3;
const GAP: u16 = 1;

/// Toasts float over the top right corner, newest underneath, each one sliding
/// out of the right edge and back into it when its time is up.
pub fn draw(frame: &mut Frame, app: &AppService, area: Rect) {
    let mut top = area.y + 1;

    for toast in &app.toasts {
        if top + HEIGHT > area.bottom() {
            break;
        }

        draw_one(frame, app, toast, area, top);
        top += HEIGHT + GAP;
    }
}

fn draw_one(frame: &mut Frame, app: &AppService, toast: &Toast, area: Rect, top: u16) {
    let t = &app.theme;

    let (icon, color) = match toast.kind {
        ToastKind::Success => ("✔", &t.success),
        ToastKind::Error => ("✖", &t.error),
    };

    let full_width = (toast.message.chars().count() as u16 + 6)
        .min(MAX_WIDTH)
        .min(area.width);

    // INFO: the corner is fixed and the width is animated, which is as close
    // to sliding in from off screen as a terminal gets
    let width = (full_width as f32 * toast.openness()).round() as u16;
    if width < 4 {
        return;
    }

    let rect = Rect { x: area.right().saturating_sub(width), y: top, width, height: HEIGHT };

    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color.to_color()))
        .padding(Padding::horizontal(1))
        .style(t.base());

    let text = Line::from(vec![
        Span::styled(format!("{} ", icon), Style::default().fg(color.to_color())),
        Span::styled(&toast.message, t.base()),
    ]);

    frame.render_widget(Paragraph::new(text).block(block), rect);
    draw_life_bar(frame, toast, rect, color.to_color());
}

/// The bottom border doubles as the countdown, so the toast shows how long it
/// has left without spending a row on a progress bar.
fn draw_life_bar(frame: &mut Frame, toast: &Toast, rect: Rect, color: ratatui::style::Color) {
    let track = rect.width.saturating_sub(2);
    let filled = (track as f32 * toast.remaining()).round() as u16;
    let y = rect.bottom() - 1;

    let buffer = frame.buffer_mut();
    for i in 0..filled {
        buffer
            .get_mut(rect.x + 1 + i, y)
            .set_symbol("━")
            .set_style(Style::default().fg(color));
    }
}
