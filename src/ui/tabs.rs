use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::Focus;
use crate::services::AppService;

const CLOSE: &str = "×";

/// The powerline points that finish a tab off at each end.
const LEFT_CAP: &str = "\u{e0b2}";
const RIGHT_CAP: &str = "\u{e0b0}";

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
        // the two caps, then a space before the next tab
        x += label.chars().count() as u16 + 3;
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
            if column >= x + width_of(&label) - 3 {
                TabHit::Close(index)
            } else {
                TabHit::Select(index)
            }
        })
}

fn width_of(label: &str) -> u16 {
    label.chars().count() as u16 + 2
}

/// Tabs are cards of their own: the one you are looking at is filled in and
/// the rest are outlined, the same way the buttons in the popups are.
/// Tabs are pills of their own: the one you are looking at is filled in and
/// the rest sit back on a quieter surface.
pub fn draw(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;
    let mut spans = vec![Span::raw(" ")];

    for (_, label, index) in layout(app) {
        let is_active = app.active_tab == Some(index);

        let (colour, fill) = match (is_active, app.focus) {
            (true, Focus::Session) => (t.accent.to_color(), t.pill(&t.accent)),
            (true, Focus::Sidebar) => (t.selected_bg.to_color(), t.selected()),
            _ => (t.input_bg.to_color(), t.surface()),
        };

        spans.extend(pill(&label, colour, fill, t));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).style(t.base()), area);
}

/// A label with both ends brought to a point. The caps are drawn in the colour
/// the tab is filled with, on the page behind it, which is what shapes them.
fn pill<'a>(
    label: &str,
    colour: ratatui::style::Color,
    fill: ratatui::style::Style,
    t: &crate::models::Theme,
) -> Vec<Span<'a>> {
    let cap = t.base().fg(colour);

    vec![
        Span::styled(LEFT_CAP, cap),
        Span::styled(label.to_string(), fill),
        Span::styled(RIGHT_CAP, cap),
        Span::raw(" "),
    ]
}
