use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::Block,
    Frame,
};

use crate::models::Mode;
use crate::services::AppService;

use super::panels;
use super::popups;
use super::toasts;

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
    toasts::draw(frame, app, body);
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
        .spacing(1)
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
    use std::time::Duration;

    use crate::models::Toast;
    use crate::test_support::{app_with, host};
    use crate::ui::screenshot;

    fn hosts(count: usize) -> Vec<crate::models::SshHost> {
        (1..=count).map(|i| host(&format!("server-{:02}", i), 22)).collect()
    }

    #[test]
    fn a_toast_opens_out_of_the_top_right_corner() {
        let (mut app, _repo) = app_with(hosts(4));
        app.toasts.push(Toast::success("Added 'prod-web'"));
        app.advance_toasts(Duration::from_millis(40));

        let sliding = screenshot::draw(&app, 80, 18);
        assert!(
            !sliding.contains("Added 'prod-web'"),
            "the toast arrived at full width instead of opening:\n{}",
            sliding
        );

        app.advance_toasts(Duration::from_millis(200));
        let open = screenshot::draw(&app, 80, 18);
        let row = open
            .lines()
            .find(|line| line.contains("✔"))
            .unwrap_or_else(|| panic!("the toast never opened:\n{}", open));

        assert!(row.contains("Added 'prod-web'"), "the message is cut off:\n{}", open);
        assert!(row.ends_with('│'), "the toast is not against the right edge:\n{}", open);
    }

    #[test]
    fn a_toast_leaves_without_anyone_pressing_a_key() {
        let (mut app, _repo) = app_with(hosts(4));
        app.toasts.push(Toast::success("Added 'prod-web'"));

        app.advance_toasts(Duration::from_secs(6));

        assert!(!app.has_toasts(), "the toast outstayed its lifetime");
        assert!(!screenshot::draw(&app, 80, 18).contains("Added"));
    }

    #[test]
    fn a_host_reads_as_a_two_line_card() {
        let (app, _repo) = app_with(hosts(3));

        let screen = screenshot::draw(&app, 80, 24);
        let lines: Vec<&str> = screen.lines().collect();
        let marked = lines
            .iter()
            .position(|line| line.contains("┃ server-01"))
            .unwrap_or_else(|| panic!("the selected card is not marked:\n{}", screen));

        assert!(
            lines[marked + 1].contains("dperez@server-01.example.com"),
            "the card is missing its target line:\n{}",
            screen
        );
    }

    #[test]
    fn a_custom_port_and_a_key_sit_beside_the_host() {
        let mut list = hosts(2);
        list[0].port = 2222;
        list[0].identity_file = "~/.ssh/id_ed25519".into();

        let (app, _repo) = app_with(list);
        let screen = screenshot::draw(&app, 80, 24);
        let card = screen
            .lines()
            .find(|line| line.contains("┃ server-01"))
            .unwrap_or_else(|| panic!("the selected card is missing:\n{}", screen));

        assert!(card.contains(":2222"), "the port badge is missing:\n{}", screen);
        assert!(screen.contains("id_ed25519"), "the key is missing:\n{}", screen);
    }

    #[test]
    fn an_empty_config_says_what_to_press() {
        let (app, _repo) = app_with(vec![]);

        let screen = screenshot::draw(&app, 80, 24);

        assert!(screen.contains("No hosts yet"), "empty state is missing:\n{}", screen);
        assert!(screen.contains("Press 'a' to add"), "the way forward is missing:\n{}", screen);
    }

    #[test]
    fn the_selected_host_stays_on_screen_in_a_long_list() {
        let (mut app, _repo) = app_with(hosts(40));
        app.jump_to_bottom();

        let screen = screenshot::draw(&app, 80, 24);

        assert!(screen.contains("┃ server-40"), "selection scrolled out of view:\n{}", screen);
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
