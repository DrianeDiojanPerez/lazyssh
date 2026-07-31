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

fn badge_width(app: &AppService) -> u16 {
    BADGE.chars().count() as u16 + 1 + u16::from(app.tab_edges())
}

/// The half triangles that slant a tab off at each end. The left one cuts away
/// with a backslash, so the fill runs from the top left corner down to meet the
/// label, and the right one leans the same way out of it.
const TAB_LEFT: &str = "\u{e0be}";
const TAB_RIGHT: &str = "\u{e0b8}";

/// The tabs sit on the middle line of the row, with a blank line above and
/// below them.
fn label_row(area: Rect) -> u16 {
    area.y + 1
}

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
    let mut x = badge_width(app);
    let edges = u16::from(app.tab_edges()) * 2;

    for (index, session) in app.sessions.iter().enumerate() {
        // a session that has ended keeps its tab until it is closed, so the
        // last thing it printed can still be read
        let mark = if session.is_running() { "" } else { "·" };
        let label = format!(" {}{} {} ", mark, session.alias, CLOSE);

        placed.push((x, label.clone(), index));
        // the slants if they are on, then a space before the next tab
        x += label.chars().count() as u16 + edges + 1;
    }

    placed
}

pub fn tab_at(app: &AppService, area: Rect, column: u16, row: u16) -> Option<TabHit> {
    if row != label_row(area) {
        return None;
    }

    let column = column.checked_sub(area.x)?;

    layout(app)
        .into_iter()
        .find(|(x, label, _)| column >= *x && column < x + width_of(app, label))
        .map(|(x, label, index)| {
            // the cross keeps the right hand end of the tab to itself
            if column >= x + width_of(app, &label) - 2 - u16::from(app.tab_edges()) {
                TabHit::Close(index)
            } else {
                TabHit::Select(index)
            }
        })
}

fn width_of(app: &AppService, label: &str) -> u16 {
    label.chars().count() as u16 + u16::from(app.tab_edges()) * 2
}

/// Tabs are cards of their own: the one you are looking at is filled in and
/// the rest are outlined, the same way the buttons in the popups are.
/// Tabs are pills of their own: the one you are looking at is filled in and
/// the rest sit back on a quieter surface.
pub fn draw(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;
    let edges = app.tab_edges();
    let mut spans = label_pill(
        BADGE,
        t.accent_secondary.to_color(),
        t.pill(&t.accent_secondary),
        t,
        edges,
    );

    for (_, label, index) in layout(app) {
        let is_active = app.active_tab == Some(index);

        let (colour, fill) = match (is_active, app.focus) {
            (true, Focus::Session) => (t.accent.to_color(), t.pill(&t.accent)),
            (true, Focus::Sidebar) => (t.selected_bg.to_color(), t.selected()),
            _ => (t.input_bg.to_color(), t.surface()),
        };

        spans.extend(tab_pill(&label, colour, fill, t, edges));
    }

    let lines = vec![Line::from(""), Line::from(spans)];
    frame.render_widget(Paragraph::new(lines).style(t.base()), area);
}

/// The row's label, slanted on its right the same way a tab is, and left as a
/// plain block when the tabs are.
fn label_pill<'a>(
    label: &str,
    colour: ratatui::style::Color,
    fill: ratatui::style::Style,
    t: &crate::models::Theme,
    edges: bool,
) -> Vec<Span<'a>> {
    let mut spans = vec![Span::styled(label.to_string(), fill)];

    if edges {
        spans.push(Span::styled(TAB_RIGHT, t.base().fg(colour)));
    }
    spans.push(Span::raw(" "));

    spans
}

/// A tab, slanted at both ends. The slants are drawn in the colour the tab is
/// filled with, on the page behind it, which is what shapes them.
fn tab_pill<'a>(
    label: &str,
    colour: ratatui::style::Color,
    fill: ratatui::style::Style,
    t: &crate::models::Theme,
    edges: bool,
) -> Vec<Span<'a>> {
    if !edges {
        return vec![Span::styled(label.to_string(), fill), Span::raw(" ")];
    }

    let slant = t.base().fg(colour);

    vec![
        Span::styled(TAB_LEFT, slant),
        Span::styled(label.to_string(), fill),
        Span::styled(TAB_RIGHT, slant),
        Span::raw(" "),
    ]
}
