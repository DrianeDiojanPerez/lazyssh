use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

use crate::models::{Toast, ToastKind};
use crate::services::AppService;

const MAX_WIDTH: u16 = 46;
const MIN_WIDTH: u16 = 24;
const HEIGHT: u16 = 5;
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

    // the filled circle glyphs nvim-notify uses, so the icon reads as a badge
    let (icon, title, color) = match toast.kind {
        ToastKind::Success => ("\u{f05a}", "Success", &t.success),
        ToastKind::Error => ("\u{f057}", "Error", &t.error),
    };

    let heading = format!("{} {}", icon, title);
    let content = (heading.chars().count() + toast.at.chars().count() + 2)
        .max(toast.message.chars().count());

    // the borders and a column of air on each side
    let full_width = (content as u16 + 4).clamp(MIN_WIDTH, MAX_WIDTH).min(area.width);

    // INFO: the corner is fixed and the width is animated, which is as close
    // to sliding in from off screen as a terminal gets
    let width = (full_width as f32 * toast.openness()).round() as u16;
    if width < 6 {
        return;
    }

    let rect = Rect { x: area.right().saturating_sub(width), y: top, width, height: HEIGHT };
    frame.render_widget(Clear, rect);

    // INFO: the whole frame takes the level colour, which is what makes a
    // toast readable as good or bad news before a word of it is read
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color.to_color()))
        .padding(Padding::horizontal(1))
        .style(t.base());

    let text_width = rect.width.saturating_sub(4) as usize;

    let lines = vec![
        Line::from(vec![
            Span::styled(
                ellipsize(&heading, text_width),
                Style::default().fg(color.to_color()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(stamp(toast, &heading, text_width), t.muted()),
        ]),
        // the divider is painted straight onto the frame afterwards so it can
        // run into the side borders instead of stopping short of them
        Line::from(""),
        Line::from(Span::styled(
            ellipsize(&toast.message, text_width),
            t.base().add_modifier(Modifier::BOLD),
        )),
    ];

    frame.render_widget(Paragraph::new(lines).block(block), rect);
    draw_divider(frame, rect, color.to_color());
    draw_life_bar(frame, toast, rect, color.to_color());
}

/// The clock sits hard right on the title row, and gives way entirely once the
/// toast is too narrow to hold both it and the heading.
fn stamp(toast: &Toast, heading: &str, text_width: usize) -> String {
    let room = text_width.saturating_sub(heading.chars().count());
    if room < toast.at.chars().count() + 1 {
        return String::new();
    }

    format!("{:>width$}", toast.at, width = room)
}

/// Splits the title row off from the message, tying into the side borders and
/// wearing their colour so the toast reads as one piece.
fn draw_divider(frame: &mut Frame, rect: Rect, color: ratatui::style::Color) {
    let style = Style::default().fg(color);
    let y = rect.y + 2;

    let buffer = frame.buffer_mut();
    buffer.get_mut(rect.x, y).set_symbol("├").set_style(style);
    buffer.get_mut(rect.right() - 1, y).set_symbol("┤").set_style(style);

    for x in (rect.x + 1)..(rect.right() - 1) {
        buffer.get_mut(x, y).set_symbol("─").set_style(style);
    }
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

fn ellipsize(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit.saturating_sub(1)).chain("…".chars()).collect()
}
