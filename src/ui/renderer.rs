use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Block,
    Frame,
};

use crate::models::Mode;
use crate::services::AppService;

use super::panels;
use super::popups;
use super::session;
use super::tabs;
use super::toasts;

/// Where each part of the screen sits. Worked out once so that drawing and
/// mouse hit testing can never disagree about what is where.
pub struct Frames {
    pub header: Rect,
    pub tabs: Option<Rect>,
    pub body: Rect,
    pub sidebar: Option<Rect>,
    pub search: Rect,
    pub list: Rect,
    pub main: Rect,
    pub status: Rect,
}

/// The sidebar is a fixed strip rather than a share of the width, so the
/// session beside it keeps its size as the window grows.
const SIDEBAR: u16 = 40;

pub fn frames(app: &AppService, area: Rect) -> Frames {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(if app.sessions.is_empty() { 0 } else { 1 }),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    let body = rows[2];
    let width = if app.sidebar_open {
        SIDEBAR.min(body.width.saturating_sub(20)).max(0)
    } else {
        0
    };

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(width), Constraint::Min(10)])
        .spacing(if width > 0 { 1 } else { 0 })
        .split(body);

    // the filter box lives at the top of the sidebar and stays there, so the
    // list never jumps when a search begins
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(columns[0]);

    Frames {
        header: rows[0],
        tabs: (!app.sessions.is_empty()).then_some(rows[1]),
        body,
        sidebar: (width > 0).then_some(columns[0]),
        search: sidebar[0],
        list: sidebar[1],
        main: columns[1],
        status: rows[3],
    }
}

pub fn render(frame: &mut Frame, app: &AppService) {
    let area = frame.size();

    frame.render_widget(Block::default().style(app.theme.base()), area);

    let frames = frames(app, area);

    panels::draw_header(frame, app, frames.header);
    if let Some(tabs) = frames.tabs {
        tabs::draw(frame, app, tabs);
    }
    if frames.sidebar.is_some() {
        panels::draw_search_bar(frame, app, frames.search);
        panels::draw_host_list(frame, app, frames.list);
    }

    match app.active_session() {
        Some(session) => session::draw(frame, app, session, frames.main),
        None => panels::draw_detail_panel(frame, app, frames.main),
    }

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

    use crossterm::event::KeyCode;
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

    /// The column a piece of text starts at on screen, which is what the mouse
    /// would report if it were clicked.
    fn column_of(screen: &str, row: u16, needle: &str) -> u16 {
        let line = screen.lines().nth(row as usize).expect("row is off screen");
        let at = line
            .find(needle)
            .unwrap_or_else(|| panic!("'{}' is not on row {}:\n{}", needle, row, screen));

        line[..at].chars().count() as u16
    }

    fn row_of(screen: &str, needle: &str) -> u16 {
        screen
            .lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("'{}' is nowhere on screen:\n{}", needle, screen)) as u16
    }

    #[test]
    fn the_status_bar_hints_are_buttons() {
        let (app, _repo) = app_with(hosts(3));
        let frames = super::frames(&app, Rect::new(0, 0, 80, 24));
        let screen = screenshot::draw(&app, 80, 24);
        let bar = frames.status.y;

        for (hint, code) in [
            ("a add", KeyCode::Char('a')),
            ("d delete", KeyCode::Char('d')),
            ("? help", KeyCode::Char('?')),
        ] {
            let column = column_of(&screen, bar, hint);
            assert_eq!(
                crate::ui::panels::hint_at(&app, frames.status, column, bar),
                Some(code),
                "clicking '{}' should press its key:\n{}",
                hint,
                screen
            );
        }
    }

    #[test]
    fn the_form_answers_to_clicks_on_its_fields_and_buttons() {
        let (mut app, _repo) = app_with(hosts(3));
        app.begin_add();

        let body = super::frames(&app, Rect::new(0, 0, 80, 24)).body;
        let screen = screenshot::draw(&app, 80, 24);

        let row = row_of(&screen, "Port  default 22");
        let column = column_of(&screen, row, "Port");
        assert_eq!(
            crate::ui::popups::field_at(&app, body, column, row),
            Some(crate::models::FormField::Port),
            "clicking the Port row should focus it:\n{}",
            screen
        );

        let row = row_of(&screen, " Save ");
        assert_eq!(
            crate::ui::popups::form_button_at(&app, body, column_of(&screen, row, " Save ") + 1, row),
            Some(crate::ui::popups::FormButton::Save),
            "the Save button is not where it is drawn:\n{}",
            screen
        );
        assert_eq!(
            crate::ui::popups::form_button_at(&app, body, column_of(&screen, row, " Cancel ") + 1, row),
            Some(crate::ui::popups::FormButton::Cancel),
            "the Cancel button is not where it is drawn:\n{}",
            screen
        );
    }

    #[test]
    fn the_delete_popup_answers_to_clicks_on_its_buttons() {
        let (mut app, _repo) = app_with(hosts(3));
        app.begin_delete();

        let body = super::frames(&app, Rect::new(0, 0, 80, 24)).body;
        let screen = screenshot::draw(&app, 80, 24);
        // the title says "Delete host" too, so the buttons are found by the
        // one word that only appears on their row
        let row = row_of(&screen, " Keep ");

        assert_eq!(
            crate::ui::popups::delete_button_at(body, column_of(&screen, row, " Delete ") + 1, row),
            Some(crate::ui::popups::DeleteButton::Delete),
            "the Delete button is not where it is drawn:\n{}",
            screen
        );
        assert_eq!(
            crate::ui::popups::delete_button_at(body, column_of(&screen, row, " Keep ") + 1, row),
            Some(crate::ui::popups::DeleteButton::Keep),
            "the Keep button is not where it is drawn:\n{}",
            screen
        );
    }

    #[test]
    fn the_key_menu_answers_to_a_click() {
        let (mut app, _repo) = app_with(hosts(3));
        app.begin_add();
        app.form_field = crate::models::FormField::IdentityFile;

        let body = super::frames(&app, Rect::new(0, 0, 80, 24)).body;
        let screen = screenshot::draw(&app, 80, 24);
        let row = row_of(&screen, "~/.ssh/id_rsa");
        let column = column_of(&screen, row, "~/.ssh/id_rsa");

        assert_eq!(
            crate::ui::popups::suggestion_at(&app, body, column, row),
            Some(1),
            "clicking a key should pick that key:\n{}",
            screen
        );
    }

    fn settle(done: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(std::time::Instant::now() < deadline, "nothing settled in time");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn the_filter_is_on_screen_before_anyone_asks_for_it() {
        let (app, _repo) = app_with(hosts(3));

        let screen = screenshot::draw(&app, 80, 24);

        assert!(screen.contains("Filter"), "the filter box is missing:\n{}", screen);
        assert!(
            screen.contains("type to narrow the list"),
            "the empty filter says nothing:\n{}",
            screen
        );
    }

    #[test]
    fn shutting_the_sidebar_gives_its_room_to_the_rest() {
        let (mut app, _repo) = app_with(hosts(3));
        let area = Rect::new(0, 0, 80, 24);

        let open = super::frames(&app, area);
        app.toggle_sidebar();
        let shut = super::frames(&app, area);

        assert!(open.sidebar.is_some(), "the sidebar starts open");
        assert!(shut.sidebar.is_none(), "the sidebar should be gone");
        assert!(
            shut.main.width > open.main.width,
            "the main pane should have taken the room: {} then {}",
            open.main.width,
            shut.main.width
        );
        assert!(!screenshot::draw(&app, 80, 24).contains("Filter"));
    }

    #[test]
    fn an_open_session_takes_a_tab_and_the_main_pane() {
        let (mut app, _repo) = app_with(hosts(3));
        app.sessions.push(
            crate::services::Session::spawn(
                "server-01",
                "echo",
                &["connected".to_string()],
                20,
                40,
            )
            .expect("the pty should have started"),
        );
        app.select_tab(0);

        settle(|| screenshot::draw(&app, 100, 20).contains("connected"));

        let screen = screenshot::draw(&app, 100, 20);
        let frames = super::frames(&app, Rect::new(0, 0, 100, 20));
        let tabs = frames.tabs.expect("an open session should raise the tab bar");

        assert_eq!(
            crate::ui::tabs::tab_at(&app, tabs, column_of(&screen, tabs.y, "server-01"), tabs.y),
            Some(crate::ui::tabs::TabHit::Select(0)),
            "clicking the tab should pick it:\n{}",
            screen
        );
        assert_eq!(
            crate::ui::tabs::tab_at(&app, tabs, column_of(&screen, tabs.y, "×"), tabs.y),
            Some(crate::ui::tabs::TabHit::Close(0)),
            "clicking the cross should close it:\n{}",
            screen
        );
    }

    #[test]
    fn a_click_picks_the_card_it_lands_on() {
        let (mut app, _repo) = app_with(hosts(40));
        app.jump_to_bottom();

        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let screen = screenshot::draw(&app, 80, 24);
        let drawn = alias_rows(&screen);
        assert!(drawn.len() > 2, "expected a panel full of cards:\n{}", screen);

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

        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;

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
    fn the_selected_card_is_the_one_in_the_accent_colour() {
        let (mut app, _repo) = app_with(hosts(3));
        app.move_cursor_down();

        let screen = screenshot::draw(&app, 80, 24);
        let buffer = screenshot::buffer(&app, 80, 24);
        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;

        let edge_of = |alias: &str| {
            let row = row_of(&screen, alias) - 1;
            buffer.get(list.x + 2, row).style().fg
        };

        assert_eq!(
            edge_of("│ server-02"),
            Some(app.theme.accent.to_color()),
            "the selected card should be drawn in the accent colour:\n{}",
            screen
        );
        assert_eq!(
            edge_of("│ server-01"),
            Some(app.theme.border.to_color()),
            "an unselected card should keep the plain border:\n{}",
            screen
        );
    }

    #[test]
    fn a_host_is_drawn_as_a_boxed_card() {
        let (app, _repo) = app_with(hosts(3));

        let screen = screenshot::draw(&app, 80, 24);
        let lines: Vec<&str> = screen.lines().collect();
        let top = lines
            .iter()
            .position(|line| line.contains("│ server-01"))
            .unwrap_or_else(|| panic!("the card is missing:\n{}", screen));

        assert!(lines[top - 1].contains("╭──"), "the card has no top edge:\n{}", screen);
        assert!(
            lines[top + 1].contains("dperez@server-01.example.com"),
            "the card is missing its detail line:\n{}",
            screen
        );
        assert!(lines[top + 2].contains("╰──"), "the card has no bottom edge:\n{}", screen);
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
            .position(|line| line.contains("│ server-01"))
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

        assert!(screen.contains("│ server-40"), "selection scrolled out of view:\n{}", screen);
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
