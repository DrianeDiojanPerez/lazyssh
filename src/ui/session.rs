use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders},
    Frame,
};

use crate::services::{AppService, Session};

/// Paints what the remote end has drawn. The pty is kept the same size as this
/// pane, so the two agree on where everything is without any reflowing here.
pub fn draw(frame: &mut Frame, app: &AppService, session: &Session, area: Rect) {
    let t = &app.theme;
    let focused = app.is_session_focused();

    let title = if session.is_running() {
        format!(" {} ", session.alias)
    } else {
        format!(" {} · ended, w closes it ", session.alias)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused { t.border_focused() } else { t.border() })
        .title(Span::styled(title, if focused { t.title() } else { t.muted() }))
        .style(t.base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Ok(parser) = session.screen.lock() else {
        return;
    };
    let screen = parser.screen();

    let buffer = frame.buffer_mut();
    for row in 0..inner.height {
        // INFO: ssh puts its own notices out as lines opening with a star, and
        // says them in plain text; they are worth the warning colour
        let notice = is_notice(screen, row, inner.width);
        // INFO: a plain shell prompt is the other thing worth picking out of
        // the wall of text, so the eye can find where each command started
        let prompt = prompt_end(screen, row, inner.width);

        for column in 0..inner.width {
            let Some(cell) = screen.cell(row, column) else {
                continue;
            };

            let target = buffer.get_mut(inner.x + column, inner.y + row);
            let contents = cell.contents();

            let mut style = style_of(cell, t);
            if cell.fgcolor() == vt100::Color::Default {
                if notice {
                    style = style.fg(t.warning.to_color());
                } else if prompt.is_some_and(|end| column <= end) {
                    style = style.fg(t.accent.to_color()).add_modifier(Modifier::BOLD);
                }
            }

            target.set_symbol(if contents.is_empty() { " " } else { &contents });
            target.set_style(style);
        }
    }

    if focused && !screen.hide_cursor() {
        let (row, column) = screen.cursor_position();
        if row < inner.height && column < inner.width {
            frame.set_cursor(inner.x + column, inner.y + row);
        }
    }
}

/// A line the remote wrote in plain text, opening with a star: ssh's banner
/// and its warnings look like this and nothing else on a prompt does.
fn is_notice(screen: &vt100::Screen, row: u16, width: u16) -> bool {
    let mut seen = String::new();

    for column in 0..width.min(4) {
        match screen.cell(row, column) {
            Some(cell) => seen.push_str(&cell.contents()),
            None => break,
        }
    }

    seen.trim_start().starts_with('*')
}

/// Where a shell prompt ends, if the line opens with one. The name and the
/// host before the sign are what tell a prompt from a stray dollar in output.
fn prompt_end(screen: &vt100::Screen, row: u16, width: u16) -> Option<u16> {
    let mut named = false;

    for column in 0..width {
        let cell = screen.cell(row, column)?;

        let contents = cell.contents();

        match contents.trim() {
            "@" => named = true,
            "$" | "#" if named => return Some(column),
            _ => {}
        }
    }

    None
}

/// The remote's own colours are kept as they are; what it leaves at the
/// default is drawn in the theme, so a session looks part of the app.
fn style_of(cell: &vt100::Cell, t: &crate::models::Theme) -> Style {
    let fg = match cell.fgcolor() {
        vt100::Color::Default => t.base().fg.unwrap_or(Color::Reset),
        colour => color_of(colour),
    };
    let bg = match cell.bgcolor() {
        vt100::Color::Default => t.base().bg.unwrap_or(Color::Reset),
        colour => color_of(colour),
    };

    let mut style = Style::default().fg(fg).bg(bg);

    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }

    style
}

fn color_of(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(index) => Color::Indexed(index),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
