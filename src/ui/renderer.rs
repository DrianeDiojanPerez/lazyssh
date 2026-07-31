use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Block,
    Frame,
};

use crate::models::Mode;
use crate::services::AppService;

use super::panels;
use super::popups;
use super::toasts;

/// Where each part of the screen sits. Worked out once so that drawing and
/// mouse hit testing can never disagree about what is where.
pub struct Frames {
    pub body: Rect,
    pub search: Option<Rect>,
    pub list: Rect,
    pub details: Rect,
    pub header: Rect,
    pub status: Rect,
}

pub fn frames(area: Rect, searching: bool) -> Frames {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .spacing(1)
        .split(rows[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if searching { 3 } else { 0 }),
            Constraint::Min(5),
        ])
        .split(columns[0]);

    Frames {
        body: rows[1],
        search: searching.then_some(left[0]),
        list: left[1],
        details: columns[1],
        header: rows[0],
        status: rows[2],
    }
}

pub fn render(frame: &mut Frame, app: &AppService) {
    let area = frame.size();

    frame.render_widget(Block::default().style(app.theme.base()), area);

    let frames = frames(area, app.mode == Mode::Search);

    panels::draw_header(frame, app, frames.header);
    if let Some(search) = frames.search {
        panels::draw_search_bar(frame, app, search);
    }
    panels::draw_host_list(frame, app, frames.list);
    panels::draw_detail_panel(frame, app, frames.details);
    panels::draw_status_bar(frame, app, frames.status);

    // INFO: toasts belong to the screen, not to the panels, so they hang in
    // the very corner rather than starting where the details do
    toasts::draw(frame, app, area);

    let body = frames.body;
    match &app.mode {
        Mode::AddHost => popups::draw_form(frame, app, " Add host ", body),
        Mode::EditHost(_) => popups::draw_form(frame, app, " Edit host ", body),
        Mode::ConfirmDelete(idx) => popups::draw_delete_confirmation(frame, app, *idx, body),
        Mode::SelectTheme => popups::draw_theme_selector(frame, app, body),
        Mode::Help => popups::draw_help(frame, app, body),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::layout::Rect;

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

        app.advance_toasts(Duration::from_millis(300));
        let open = screenshot::draw(&app, 80, 18);
        let title = open
            .lines()
            .find(|line| line.contains("Success"))
            .unwrap_or_else(|| panic!("the toast never opened:\n{}", open));

        assert!(title.ends_with('│'), "the toast is not against the right edge:\n{}", open);
        assert!(
            open.lines().any(|line| line.contains("Added 'prod-web'")),
            "the message is cut off:\n{}",
            open
        );
    }

    #[test]
    fn a_toast_leaves_without_anyone_pressing_a_key() {
        let (mut app, _repo) = app_with(hosts(4));
        app.toasts.push(Toast::success("Added 'prod-web'"));

        app.advance_toasts(Duration::from_secs(6));

        assert!(!app.has_toasts(), "the toast outstayed its lifetime");
        assert!(!screenshot::draw(&app, 80, 18).contains("Added"));
    }

    /// The alias line of each card, as drawn: "│ server-07    :22 │".
    fn alias_rows(screen: &str) -> Vec<(u16, usize)> {
        screen
            .lines()
            .enumerate()
            .filter_map(|(row, line)| {
                let text = line.trim_start_matches(['│', '┃', ' ']);
                let number = text.strip_prefix("server-")?.get(..2)?.parse::<usize>().ok()?;
                (!text.contains('@')).then_some((row as u16, number - 1))
            })
            .collect()
    }

    #[test]
    fn the_toast_divider_meets_both_borders() {
        let (mut app, _repo) = app_with(hosts(4));
        app.toasts.push(Toast::success("Added 'prod-web'"));
        app.advance_toasts(Duration::from_millis(300));

        let screen = screenshot::draw(&app, 80, 18);
        let divider = screen
            .lines()
            .find(|line| line.contains('├'))
            .unwrap_or_else(|| panic!("the toast has no divider:\n{}", screen));

        let (_, toast_part) = divider.split_once('├').unwrap();
        assert!(toast_part.ends_with('┤'), "the divider stops short of the border:\n{}", screen);
        assert!(
            !toast_part.contains(' '),
            "the divider has a gap in it:\n{}",
            screen
        );
    }

    #[test]
    fn the_identity_field_offers_the_keys_on_disk() {
        let (mut app, _repo) = app_with(hosts(3));
        app.begin_add();
        app.form_field = crate::models::FormField::IdentityFile;

        let screen = screenshot::draw(&app, 80, 24);

        assert!(screen.contains("keys"), "the menu never opened:\n{}", screen);
        assert!(screen.contains("~/.ssh/id_ed25519"), "a key is missing:\n{}", screen);
        assert!(screen.contains("IdentityFile"), "the menu covers its own field:\n{}", screen);

        for c in "work".chars() {
            app.form_type_char(c);
        }
        let filtered = screenshot::draw(&app, 80, 24);

        assert!(filtered.contains("work_ed255"), "the match is missing:\n{}", filtered);
        assert!(!filtered.contains("id_rsa"), "the menu should have narrowed:\n{}", filtered);
    }

    #[test]
    fn a_click_picks_the_card_it_lands_on() {
        let (mut app, _repo) = app_with(hosts(40));
        app.jump_to_bottom();

        let list = super::frames(Rect::new(0, 0, 80, 24), false).list;
        let screen = screenshot::draw(&app, 80, 24);
        let drawn = alias_rows(&screen);
        assert!(drawn.len() > 3, "expected a full panel of cards:\n{}", screen);

        for (row, index) in drawn {
            assert_eq!(
                crate::ui::panels::host_at(&app, list, list.x + 2, row),
                Some(index),
                "clicking row {} should pick server-{:02}:\n{}",
                row,
                index + 1,
                screen
            );
        }
    }

    #[test]
    fn a_click_on_empty_space_picks_nothing() {
        let (app, _repo) = app_with(hosts(2));

        let list = super::frames(Rect::new(0, 0, 80, 24), false).list;

        assert_eq!(
            crate::ui::panels::host_at(&app, list, list.x + 2, list.bottom() - 2),
            None,
            "the space below the last card is not a host"
        );
        assert_eq!(
            crate::ui::panels::host_at(&app, list, list.right() + 4, list.y + 2),
            None,
            "the details panel is not the host list"
        );
    }

    #[test]
    fn a_host_is_drawn_as_a_boxed_card() {
        let (app, _repo) = app_with(hosts(3));

        let screen = screenshot::draw(&app, 80, 24);
        let lines: Vec<&str> = screen.lines().collect();
        let top = lines
            .iter()
            .position(|line| line.contains("┃ server-01"))
            .unwrap_or_else(|| panic!("the selected card is missing:\n{}", screen));

        assert!(lines[top - 1].contains("┏━━"), "the card has no top edge:\n{}", screen);
        assert!(
            lines[top + 1].contains("dperez@server-01.example.com"),
            "the card is missing its detail line:\n{}",
            screen
        );
        assert!(lines[top + 2].contains("┗━━"), "the card has no bottom edge:\n{}", screen);
        assert!(
            screen.contains("│ server-02"),
            "an unselected card should keep the light box:\n{}",
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
        let lines: Vec<&str> = screen.lines().collect();
        let top = lines
            .iter()
            .position(|line| line.contains("┃ server-01"))
            .unwrap_or_else(|| panic!("the card is missing:\n{}", screen));

        assert!(lines[top].contains(":2222"), "the port badge is missing:\n{}", screen);
        assert!(lines[top + 1].contains("id_ed25519"), "the key is missing:\n{}", screen);
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
