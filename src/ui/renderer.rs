use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::Block,
    Frame,
};

use crate::models::Mode;
use crate::services::AppService;

use super::panels;
use super::popups;

pub fn render(frame: &mut Frame, app: &AppService) {
    let area = frame.size();

    frame.render_widget(Block::default().style(app.theme.base()), area);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);

    panels::draw_header(frame, app, main_layout[0]);
    draw_body(frame, app, main_layout[1]);
    panels::draw_status_bar(frame, app, main_layout[2]);

    match &app.mode {
        Mode::AddHost => popups::draw_form(frame, app, " + Add SSH Host "),
        Mode::EditHost(_) => popups::draw_form(frame, app, " Edit SSH Host "),
        Mode::ConfirmDelete(idx) => popups::draw_delete_confirmation(frame, app, *idx),
        Mode::SelectTheme => popups::draw_theme_selector(frame, app),
        Mode::Help => popups::draw_help(frame, app),
        _ => {}
    }
}

fn draw_body(frame: &mut Frame, app: &AppService, area: ratatui::layout::Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let has_search = app.mode == Mode::Search;

    let left_panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if has_search { 3 } else { 0 }),
            Constraint::Min(5),
        ])
        .split(columns[0]);

    if has_search {
        panels::draw_search_bar(frame, app, left_panes[0]);
    }
    panels::draw_host_list(frame, app, left_panes[1]);

    let right_panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8)])
        .split(columns[1]);

    panels::draw_detail_panel(frame, app, right_panes[0]);
}
