use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::Focus;
use crate::services::AppService;

const CLOSE: &str = "×";

/// The label the row opens with, in the same shape as the tabs beside it.
const BADGE: &str = " tabs ";

fn badge_width() -> u16 {
    BADGE.chars().count() as u16 + 2
}

/// The half triangles that slant a tab off at each end: the left one leans the
/// other way up from the right one.
const TAB_LEFT: &str = "\u{e0be}";
const TAB_RIGHT: &str = "\u{e0b8}";

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
    // INFO: the row starts hard against the left edge, so nothing is indented
    // away from the corner of the screen
    let mut x = badge_width();

    for (index, session) in app.sessions.iter().enumerate() {
        // a session that has ended keeps its tab until it is closed, so the
        // last thing it printed can still be read
        let mark = if session.is_running() { "" } else { "·" };
        let label = format!(" {}{} {} ", mark, session.alias, CLOSE);

        placed.push((x, label.clone(), index));
        // a slant at each end, then a space before the next tab
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
    let mut spans = label_pill(BADGE, t.accent_secondary.to_color(), t.pill(&t.accent_secondary), t);

    for (_, label, index) in layout(app) {
        let is_active = app.active_tab == Some(index);

        let (colour, fill) = match (is_active, app.focus) {
            (true, Focus::Session) => (t.accent.to_color(), t.pill(&t.accent)),
            (true, Focus::Sidebar) => (t.selected_bg.to_color(), t.selected()),
            _ => (t.input_bg.to_color(), t.surface()),
        };

        spans.extend(tab_pill(&label, colour, fill, t));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).style(t.base()), area);
}

/// The row's label, slanted on its right the same way a tab is.
fn label_pill<'a>(
    label: &str,
    colour: ratatui::style::Color,
    fill: ratatui::style::Style,
    t: &crate::models::Theme,
) -> Vec<Span<'a>> {
    vec![
        Span::styled(label.to_string(), fill),
        Span::styled(TAB_RIGHT, t.base().fg(colour)),
        Span::raw(" "),
    ]
}

/// A tab, slanted at both ends. The slants are drawn in the colour the tab is
/// filled with, on the page behind it, which is what shapes them.
fn tab_pill<'a>(
    label: &str,
    colour: ratatui::style::Color,
    fill: ratatui::style::Style,
    t: &crate::models::Theme,
) -> Vec<Span<'a>> {
    let slant = t.base().fg(colour);

    vec![
        Span::styled(TAB_LEFT, slant),
        Span::styled(label.to_string(), fill),
        Span::styled(TAB_RIGHT, slant),
        Span::raw(" "),
    ]
}
