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
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    panels::draw_header(frame, app, main_layout[0]);
    draw_body(frame, app, main_layout[1]);
    panels::draw_status_bar(frame, app, main_layout[2]);

    let body = main_layout[1];
    match &app.mode {
        Mode::AddHost => popups::draw_form(frame, app, " Add host ", body),
        Mode::EditHost(_) => popups::draw_form(frame, app, " Edit host ", body),
        Mode::ConfirmDelete(idx) => popups::draw_delete_confirmation(frame, app, *idx, body),
        Mode::SelectTheme => popups::draw_theme_selector(frame, app, body),
        Mode::Help => popups::draw_help(frame, app, body),
        _ => {}
    }
}

fn draw_body(frame: &mut Frame, app: &AppService, area: ratatui::layout::Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
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

#[cfg(test)]
mod tests {
    use crate::test_support::{app_with, host};
    use crate::ui::screenshot;

    fn hosts(count: usize) -> Vec<crate::models::SshHost> {
        (1..=count).map(|i| host(&format!("server-{:02}", i), 22)).collect()
    }

    #[test]
    fn the_selected_host_stays_on_screen_in_a_long_list() {
        let (mut app, _repo) = app_with(hosts(40));
        app.jump_to_bottom();

        let screen = screenshot::draw(&app, 80, 24);

        assert!(screen.contains("▸ server-40"), "selection scrolled out of view:\n{}", screen);
    }

    #[test]
    fn the_form_shows_every_field_on_a_small_terminal() {
        let (mut app, _repo) = app_with(hosts(3));
        app.begin_add();

        let screen = screenshot::draw(&app, 80, 24);

        for label in ["Host Alias", "HostName", "Port", "User", "IdentityFile"] {
            assert!(screen.contains(label), "form is missing '{}':\n{}", label, screen);
        }
        assert!(screen.contains("Esc cancel"), "form is missing its footer:\n{}", screen);
    }

    #[test]
    fn the_form_scrolls_to_the_field_being_edited() {
        let (mut app, _repo) = app_with(hosts(3));
        app.begin_add();
        app.form_field = crate::models::FormField::IdentityFile;

        let screen = screenshot::draw(&app, 40, 12);

        assert!(screen.contains("IdentityFile"), "active field is off screen:\n{}", screen);
    }

    #[test]
    fn the_help_fits_a_small_terminal() {
        let (mut app, _repo) = app_with(hosts(3));
        app.open_help();

        let screen = screenshot::draw(&app, 80, 24);

        assert!(screen.contains("Esc closes this help"), "help is clipped:\n{}", screen);
    }

    #[test]
    fn an_active_filter_is_visible_once_the_search_bar_closes() {
        let (mut app, _repo) = app_with(hosts(20));
        app.enter_search();
        for c in "server-1".chars() {
            app.search_type(c);
        }
        app.finish_search();

        let screen = screenshot::draw(&app, 80, 24);

        assert!(screen.contains("10/20"), "filter counts are missing:\n{}", screen);
        assert!(screen.contains("server-1'"), "filter query is missing:\n{}", screen);
    }

    #[test]
    fn the_status_bar_keeps_the_way_out_on_a_narrow_terminal() {
        let (mut app, _repo) = app_with(hosts(20));

        assert!(screenshot::draw(&app, 50, 18).contains("? help"));

        app.begin_add();
        assert!(screenshot::draw(&app, 50, 18).contains("Esc cancel"));
    }
}
