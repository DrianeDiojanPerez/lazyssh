use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

use crate::models::{Toast, ToastKind};
use crate::services::AppService;

const MAX_WIDTH: u16 = 56;
const MIN_WIDTH: u16 = 34;
/// Two borders, the title and its divider, before the message is counted.
const CHROME: u16 = 4;
const MESSAGE_ROWS: usize = 2;
const GAP: u16 = 1;

pub fn draw(frame: &mut Frame, app: &AppService, area: Rect) {
    let mut top = area.y;

    for toast in &app.toasts {
        let (width, height) = size(toast, area.width);
        if top + height > area.bottom() {
            break;
        }

        draw_one(frame, app, toast, area, top, width, height);
        top += height + GAP;
    }
}

/// How much room a toast asks for. The height follows the message, so a short
/// one does not sit above a blank row.
fn size(toast: &Toast, available: u16) -> (u16, u16) {
    let heading = toast.kind.title().chars().count() + 2;
    let content = (heading + toast.at.chars().count() + 2)
        .max(toast.message.chars().count().min(MAX_WIDTH as usize));

    // the borders and a column of air on each side
    let width = (content as u16 + 4).clamp(MIN_WIDTH, MAX_WIDTH).min(available);
    let rows = wrap(&toast.message, width.saturating_sub(4) as usize, MESSAGE_ROWS).len();

    (width, CHROME + rows.max(1) as u16)
}

fn draw_one(
    frame: &mut Frame,
    app: &AppService,
    toast: &Toast,
    area: Rect,
    top: u16,
    full_width: u16,
    height: u16,
) {
    let t = &app.theme;

    let (icon, color) = match toast.kind {
        ToastKind::Success => ("\u{f05a}", &t.success),
        ToastKind::Error => ("\u{f057}", &t.error),
    };
    let heading = format!("{} {}", icon, toast.kind.title());

    // INFO: the corner is fixed and the width is animated, which is as close
    // to sliding in from off screen as a terminal gets
    let width = (full_width as f32 * toast.openness()).round() as u16;
    if width < 6 {
        return;
    }

    let rect = Rect { x: area.right().saturating_sub(width), y: top, width, height };
    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color.to_color()))
        .padding(Padding::horizontal(1))
        .style(t.base());

    let text_width = rect.width.saturating_sub(4) as usize;

    let mut lines = vec![
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
    ];
    for line in wrap(&toast.message, text_width, MESSAGE_ROWS) {
        lines.push(Line::from(Span::styled(line, t.base().add_modifier(Modifier::BOLD))));
    }

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

fn wrap(text: &str, width: usize, rows: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => {
                if lines.len() == rows {
                    break;
                }
                lines.push(word.to_string());
            }
        }
    }

    if let Some(last) = lines.last_mut() {
        *last = ellipsize(last, width);
    }
    lines.truncate(rows);
    lines
}

fn ellipsize(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit.saturating_sub(1)).chain("…".chars()).collect()
}
