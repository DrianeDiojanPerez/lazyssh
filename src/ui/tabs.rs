use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::Focus;
use crate::services::AppService;

const CLOSE: &str = "×";

/// What a click on the tab bar meant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabHit {
    Select(usize),
    Close(usize),
}

/// Lays the open sessions out left to right, label and all. The renderer and
/// the mouse both read this, so a tab is always where it was drawn.
fn layout(app: &AppService) -> Vec<(u16, String, usize)> {
    let mut placed = Vec::new();
    let mut x = 1;

    for (index, session) in app.sessions.iter().enumerate() {
        // a session that has ended keeps its tab until it is closed, so the
        // last thing it printed can still be read
        let mark = if session.is_running() { "" } else { "·" };
        let label = format!(" {}{} {} ", mark, session.alias, CLOSE);

        x += label.chars().count() as u16;
        placed.push((x - label.chars().count() as u16, label, index));
    }

    placed
}

pub fn tab_at(app: &AppService, area: Rect, column: u16, row: u16) -> Option<TabHit> {
    if row != area.y {
        return None;
    }

    let column = column.checked_sub(area.x)?;

    layout(app)
        .into_iter()
        .find(|(x, label, _)| {
            column >= *x && column < x + label.chars().count() as u16
        })
        .map(|(x, label, index)| {
            // the cross sits one column in from the right hand end
            if column >= x + label.chars().count() as u16 - 2 {
                TabHit::Close(index)
            } else {
                TabHit::Select(index)
            }
        })
}

pub fn draw(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    if app.sessions.is_empty() {
        // the row stays put and says what would fill it
        let empty = Line::from(Span::styled("  no open connections", t.muted()));

        frame.render_widget(Paragraph::new(empty).style(t.base()), area);
        return;
    }

    let mut spans = vec![Span::styled(" ", t.base())];

    for (_, label, index) in layout(app) {
        let is_active = app.active_tab == Some(index);

        let style = match (is_active, app.focus) {
            (true, Focus::Session) => t.pill(&t.accent),
            (true, Focus::Sidebar) => t.selected(),
            _ => t.muted(),
        };

        spans.push(Span::styled(label, style));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).style(t.base()), area);
}
