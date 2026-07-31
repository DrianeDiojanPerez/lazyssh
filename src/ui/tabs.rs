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

        placed.push((x, label.clone(), index));
        // a space between one tab and the next
        x += label.chars().count() as u16 + 1;
    }

    placed
}

pub fn tab_at(app: &AppService, area: Rect, column: u16, row: u16) -> Option<TabHit> {
    if row < area.y || row >= area.bottom() {
        return None;
    }

    let column = column.checked_sub(area.x)?;

    layout(app)
        .into_iter()
        .find(|(x, label, _)| column >= *x && column < x + width_of(label))
        .map(|(x, label, index)| {
            // the cross keeps the right hand end of the tab to itself
            if column >= x + width_of(&label) - 2 {
                TabHit::Close(index)
            } else {
                TabHit::Select(index)
            }
        })
}

fn width_of(label: &str) -> u16 {
    label.chars().count() as u16
}

/// Tabs are cards of their own: the one you are looking at is filled in and
/// the rest are outlined, the same way the buttons in the popups are.
pub fn draw(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    if app.sessions.is_empty() {
        // the row stays put and says what would fill it
        let empty = Line::from(Span::styled("  no open connections", t.muted()));


        frame.render_widget(Paragraph::new(empty).style(t.base()), area);
        return;
    }

    let mut spans = vec![Span::raw(" ")];

    for (_, label, index) in layout(app) {
        let is_active = app.active_tab == Some(index);

        // INFO: the fill is the whole of the difference, so a tab is a plain
        // block of colour with nothing drawn around it
        let fill = match (is_active, app.focus) {
            (true, Focus::Session) => t.pill(&t.accent),
            (true, Focus::Sidebar) => t.selected(),
            _ => t.surface(),
        };

        spans.push(Span::styled(label, fill));
        spans.push(Span::raw(" "));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).style(t.base()), area);
}
