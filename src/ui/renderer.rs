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
    pub tabs: Rect,
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
            // INFO: the row is always there, empty or not, so opening the first
            // connection does not shove the whole screen down a line
            Constraint::Length(if app.tab_panel() { 2 } else { 1 }),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    let body = rows[1];
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

    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(columns[0]);

    Frames {
        tabs: rows[0],
        body,
        sidebar: (width > 0).then_some(columns[0]),
        search: sidebar[0],
        list: sidebar[1],
        main: columns[1],
        status: rows[2],
    }
}

pub fn render(frame: &mut Frame, app: &AppService) {
    let area = frame.size();

    frame.render_widget(Block::default().style(app.theme.base()), area);

    let frames = frames(app, area);

    tabs::draw(frame, app, frames.tabs);
    if let Some(sidebar) = frames.sidebar {
        panels::draw_search_bar(frame, app, frames.search);
        panels::draw_host_list(frame, app, frames.list);
        panels::draw_sidebar_edge(frame, app, sidebar);
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
        Mode::ChooseLaunch => popups::draw_launch_choice(frame, app, body),
        Mode::Settings => popups::draw_settings(frame, app, body),
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

    /// The part of a row that belongs to the sidebar, so a search for an alias
    /// or a lamp cannot wander into the pane beside it.
    fn sidebar_row(screen: &str, list: Rect, row: u16) -> String {
        screen
            .lines()
            .nth(row as usize)
            .unwrap_or_default()
            .chars()
            .take(list.right() as usize)
            .collect()
    }

    /// The row a host is written on, which is the first of the two it takes.
    fn card_row(screen: &str, list: Rect, alias: &str) -> u16 {
        (list.y..list.bottom())
            .find(|row| {
                sidebar_row(screen, list, *row)
                    .trim_start_matches(['▎', ' '])
                    .starts_with(alias)
            })
            .unwrap_or_else(|| panic!("'{}' has no card:\n{}", alias, screen))
    }

    /// The alias line of each card, as drawn: "  server-07    :22  ●".
    fn alias_rows(screen: &str, list: Rect) -> Vec<(u16, usize)> {
        (list.y..list.bottom())
            .filter_map(|row| {
                let text = sidebar_row(screen, list, row);
                let text = text.trim_start_matches(['▎', ' ']);
                let number = text.strip_prefix("server-")?.get(..2)?.parse::<usize>().ok()?;
                (!text.contains('@')).then_some((row, number - 1))
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

    /// A host on a port that is actually listening, so the probe can find it.
    fn reachable() -> (crate::models::SshHost, std::net::TcpListener) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let mut host = host("localbox", port);
        host.hostname = "127.0.0.1".into();
        (host, listener)
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
    fn a_reachable_host_gets_a_green_lamp_and_a_dead_one_a_red_lamp() {
        let (up, _listener) = reachable();
        let (app, _repo) = app_with(vec![up, host("dead-host", 22)]);

        settle(|| !app.probes.is_working());

        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let screen = screenshot::draw(&app, 80, 24);
        let buffer = screenshot::buffer(&app, 80, 24);
        let lamp_on = |alias: &str| {
            let row = card_row(&screen, list, alias);
            let line = sidebar_row(&screen, list, row);
            let column = line.rfind('●').expect("the card has no lamp");
            buffer.get(line[..column].chars().count() as u16, row).style().fg
        };

        assert_eq!(
            lamp_on("localbox"),
            Some(app.theme.success.to_color()),
            "a host that answered should be lit green:\n{}",
            screen
        );
        assert_eq!(
            lamp_on("dead-host"),
            Some(app.theme.error.to_color()),
            "a host that did not answer should be lit red:\n{}",
            screen
        );
    }

    #[test]
    fn the_tab_row_holds_its_place_while_it_is_empty() {
        let (app, _repo) = app_with(hosts(3));
        let screen = screenshot::draw(&app, 100, 20);
        let row = super::frames(&app, Rect::new(0, 0, 100, 20)).tabs.y;

        assert!(
            screen.lines().nth(row as usize).is_some_and(|line| line.contains("Tabs")),
            "the panel should keep its title while it is empty:\n{}",
            screen
        );
    }

    #[test]
    fn opening_the_first_tab_moves_nothing_else() {
        let (mut app, _repo) = app_with(hosts(3));
        let area = Rect::new(0, 0, 100, 20);

        let before = super::frames(&app, area);
        app.sessions.push(
            crate::services::Session::spawn("server-01", "true", &[], 20, 40)
                .expect("the pty should have started"),
        );
        let after = super::frames(&app, area);

        assert_eq!(before.list, after.list, "the host list should not have moved");
        assert_eq!(before.main, after.main, "the main pane should not have moved");
        assert!(
            screenshot::draw(&app, 100, 20).contains("server-01"),
            "the row should be showing the tab now"
        );
    }

    #[test]
    fn a_session_wears_the_theme_and_flags_the_notices() {
        let (mut app, _repo) = app_with(hosts(3));
        app.sessions.push(
            crate::services::Session::spawn(
                "server-01",
                "printf",
                &["** WARNING: not post-quantum\nhello\n".to_string()],
                20,
                60,
            )
            .expect("the pty should have started"),
        );
        app.select_tab(0);

        settle(|| screenshot::draw(&app, 100, 20).contains("hello"));

        let screen = screenshot::draw(&app, 100, 20);
        let buffer = screenshot::buffer(&app, 100, 20);
        let colour = |needle: &str| {
            let row = row_of(&screen, needle);
            buffer.get(column_of(&screen, row, needle), row).style().fg
        };

        assert_eq!(
            colour("WARNING"),
            Some(app.theme.warning.to_color()),
            "the notice should be called out:\n{}",
            screen
        );
        assert_eq!(
            colour("hello"),
            Some(app.theme.fg.to_color()),
            "plain output should wear the theme:\n{}",
            screen
        );
    }

    #[test]
    fn a_session_picks_the_shell_prompt_out() {
        let (mut app, _repo) = app_with(hosts(3));
        app.sessions.push(
            crate::services::Session::spawn(
                "server-01",
                "printf",
                &["[dperez@webtool02 ~]$ ls\n".to_string()],
                20,
                60,
            )
            .expect("the pty should have started"),
        );
        app.select_tab(0);

        settle(|| screenshot::draw(&app, 100, 20).contains("webtool02"));

        let screen = screenshot::draw(&app, 100, 20);
        let buffer = screenshot::buffer(&app, 100, 20);
        let row = row_of(&screen, "webtool02");
        let colour = |needle: &str| {
            buffer.get(column_of(&screen, row, needle), row).style().fg
        };

        assert_eq!(
            colour("dperez"),
            Some(app.theme.accent.to_color()),
            "the prompt should stand out:\n{}",
            screen
        );
        assert_eq!(
            colour("ls"),
            Some(app.theme.fg.to_color()),
            "what was typed is not part of the prompt:\n{}",
            screen
        );
    }

    #[test]
    fn the_tabs_are_plain_until_the_slant_is_switched_on() {
        let (mut app, _repo) = app_with(hosts(3));
        app.sessions.push(
            crate::services::Session::spawn("server-01", "sleep", &["5".into()], 20, 40)
                .expect("the pty should have started"),
        );
        app.select_tab(0);

        let plain = screenshot::draw(&app, 100, 20);
        assert!(!plain.contains('\u{e0be}'), "a tab should start plain:\n{}", plain);

        app.toggle_tab_edges(&crate::test_support::StubThemeRepo);
        let slanted = screenshot::draw(&app, 100, 20);

        assert!(
            slanted.contains('\u{e0be}'),
            "the setting should put the slant back:\n{}",
            slanted
        );

        let tabs = super::frames(&app, Rect::new(0, 0, 100, 20)).tabs;
        assert_eq!(
            crate::ui::tabs::tab_at(
                &app,
                tabs,
                column_of(&slanted, tabs.y + 1, "server-01"),
                tabs.y + 1
            ),
            Some(crate::ui::tabs::TabHit::Select(0)),
        );
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
        let tabs = frames.tabs;

        let label = tabs.y + 1;
        assert_eq!(
            crate::ui::tabs::tab_at(&app, tabs, column_of(&screen, label, "server-01"), label),
            Some(crate::ui::tabs::TabHit::Select(0)),
            "clicking the tab should pick it:\n{}",
            screen
        );
        assert_eq!(
            crate::ui::tabs::tab_at(&app, tabs, column_of(&screen, label, "×"), label),
            Some(crate::ui::tabs::TabHit::Close(0)),
            "clicking the cross should close it:\n{}",
            screen
        );
        assert_eq!(
            crate::ui::tabs::tab_at(&app, tabs, column_of(&screen, label, "server-01") - 2, label),
            Some(crate::ui::tabs::TabHit::Select(0)),
            "the slanted end belongs to its tab:\n{}",
            screen
        );
    }

    #[test]
    fn the_connect_question_offers_both_ways_and_answers_to_clicks() {
        let (mut app, _repo) = app_with(hosts(3));
        app.request_connection(20, 40);

        assert_eq!(app.mode, crate::models::Mode::ChooseLaunch, "asking is the default");

        let body = super::frames(&app, Rect::new(0, 0, 84, 22)).body;
        let screen = screenshot::draw(&app, 84, 22);
        let row = row_of(&screen, "In a tab");

        let filled = |screen: &str, label: &str| {
            let row = row_of(screen, label);
            screenshot::buffer(&app, 84, 22)
                .get(column_of(screen, row, label), row)
                .style()
                .bg
        };

        assert_eq!(
            filled(&screen, "In a tab"),
            Some(app.theme.accent.to_color()),
            "the answer under the cursor should be the filled one:\n{}",
            screen
        );
        assert_eq!(
            filled(&screen, "Whole terminal"),
            Some(app.theme.input_bg.to_color()),
            "the other answer should sit back on its own surface:\n{}",
            screen
        );
        assert_eq!(
            crate::ui::popups::launch_button_at(body, column_of(&screen, row, "In a tab"), row),
            Some(crate::ui::popups::LaunchButton::Tab),
            "the tab button is not where it is drawn:\n{}",
            screen
        );
        assert_eq!(
            crate::ui::popups::launch_button_at(body, column_of(&screen, row, "Whole terminal"), row),
            Some(crate::ui::popups::LaunchButton::FullScreen),
            "the full screen button is not where it is drawn:\n{}",
            screen
        );

        app.launch_cursor_right();
        let moved = screenshot::draw(&app, 84, 22);
        let row = row_of(&moved, "Whole terminal");
        let buffer = screenshot::buffer(&app, 84, 22);

        assert_eq!(
            buffer.get(column_of(&moved, row, "Whole terminal"), row).style().bg,
            Some(app.theme.accent.to_color()),
            "the arrows should move the fill:\n{}",
            moved
        );
    }

    #[test]
    fn the_settings_rows_answer_to_clicks() {
        let (mut app, _repo) = app_with(hosts(3));
        app.open_settings();

        let body = super::frames(&app, Rect::new(0, 0, 84, 22)).body;
        let screen = screenshot::draw(&app, 84, 22);

        for (label, index) in [
            ("Take the whole terminal", 2),
            ("Theme", 3),
            ("Transparency", 4),
            ("Slanted tabs", 5),
            ("Tabs in a panel", 6),
        ] {
            assert_eq!(
                crate::ui::popups::setting_at(body, 40, row_of(&screen, label)),
                Some(index),
                "'{}' is not where it is drawn:\n{}",
                label,
                screen
            );
        }
    }

    #[test]
    fn the_bottom_bar_is_down_to_where_the_config_is() {
        let (app, _repo) = app_with(hosts(3));

        let screen = screenshot::draw(&app, 100, 12);
        let top = screen.lines().next().expect("a screen has rows");
        let bottom = screen.lines().last().expect("a screen has rows");

        assert!(top.contains("Tabs"), "the tab panel should be at the top now:\n{}", screen);
        assert!(bottom.contains(".ssh/config"), "the bar lost the config path:\n{}", screen);
        assert!(bottom.contains("3 hosts"), "the bar lost the host count:\n{}", screen);
        assert!(!bottom.contains("NORMAL"), "the mode chip should be gone:\n{}", screen);
        assert!(!bottom.contains("? help"), "the key hints should be gone:\n{}", screen);
        assert!(bottom.ends_with(' '), "the bar should sit hard right:\n{}", screen);
    }

    #[test]
    fn the_tabs_can_step_out_of_their_panel() {
        let (mut app, _repo) = app_with(hosts(3));
        app.sessions.push(
            crate::services::Session::spawn("server-01", "sleep", &["5".into()], 20, 40)
                .expect("the pty should have started"),
        );
        app.select_tab(0);

        let framed = screenshot::draw(&app, 100, 20);
        assert!(framed.contains("╭ Tabs"), "the tabs start in a panel:\n{}", framed);

        app.toggle_tab_panel(&crate::test_support::StubThemeRepo);
        let bare = screenshot::draw(&app, 100, 20);

        assert!(!bare.contains("╭ Tabs"), "the panel should be gone:\n{}", bare);
        assert!(bare.contains("tabs"), "the label takes its place:\n{}", bare);

        let tabs = super::frames(&app, Rect::new(0, 0, 100, 20)).tabs;
        assert_eq!(
            crate::ui::tabs::tab_at(&app, tabs, column_of(&bare, tabs.y, "server-01"), tabs.y),
            Some(crate::ui::tabs::TabHit::Select(0)),
            "a bare row should still answer to clicks:\n{}",
            bare
        );
    }

    #[test]
    fn a_click_picks_the_card_it_lands_on() {
        let (mut app, _repo) = app_with(hosts(40));
        app.jump_to_bottom();

        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let screen = screenshot::draw(&app, 80, 24);
        let drawn = alias_rows(&screen, list);
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
    fn the_selected_card_is_the_one_with_the_bar_down_its_side() {
        let (mut app, _repo) = app_with(hosts(3));
        app.move_cursor_down();

        let screen = screenshot::draw(&app, 80, 24);
        let buffer = screenshot::buffer(&app, 80, 24);
        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let bar_of = |alias: &str| sidebar_row(&screen, list, card_row(&screen, list, alias));

        assert!(
            bar_of("server-02").contains('▎'),
            "the selected card should be marked:\n{}",
            screen
        );
        assert!(
            !bar_of("server-01").contains('▎'),
            "only the selected card gets the bar:\n{}",
            screen
        );

        let row = card_row(&screen, list, "server-02");
        assert_eq!(
            buffer.get(list.x + 1, row).style().fg,
            Some(app.theme.accent.to_color()),
            "the bar should be drawn in the accent colour:\n{}",
            screen
        );
        assert_eq!(
            buffer.get(list.x + 1, row).style().bg,
            Some(app.theme.selected_bg.to_color()),
            "the selected card should sit on its own surface:\n{}",
            screen
        );
    }

    #[test]
    fn the_host_list_is_headed_by_a_rule_rather_than_boxed() {
        let (app, _repo) = app_with(hosts(3));

        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let screen = screenshot::draw(&app, 80, 24);
        let buffer = screenshot::buffer(&app, 80, 24);

        assert!(!screen.contains("╭ Hosts"), "the panel should be gone:\n{}", screen);

        let header = (list.y..list.bottom())
            .find(|row| sidebar_row(&screen, list, *row).contains("Hosts 3"))
            .unwrap_or_else(|| panic!("the header is missing:\n{}", screen));

        assert!(
            sidebar_row(&screen, list, header).contains("───"),
            "the header should run out to the edge:\n{}",
            screen
        );
        assert_eq!(
            buffer.get(list.right(), list.y).symbol(),
            "│",
            "the sidebar has lost the line down its side:\n{}",
            screen
        );
    }

    #[test]
    fn a_host_is_drawn_flat_on_two_lines() {
        let (app, _repo) = app_with(hosts(3));

        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let screen = screenshot::draw(&app, 80, 24);
        let top = card_row(&screen, list, "server-01");

        assert!(
            !sidebar_row(&screen, list, top).contains('╭'),
            "a card should not be boxed any more:\n{}",
            screen
        );
        assert!(
            sidebar_row(&screen, list, top + 1).contains("dperez@server-01.example.com"),
            "the card is missing its detail line:\n{}",
            screen
        );
        assert!(
            sidebar_row(&screen, list, top + 2).trim().is_empty(),
            "the cards should have air between them:\n{}",
            screen
        );
    }

    #[test]
    fn a_custom_port_and_a_key_sit_beside_the_host() {
        let mut hosts = hosts(2);
        hosts[0].port = 2222;
        hosts[0].identity_file = "~/.ssh/id_ed25519".into();

        let (app, _repo) = app_with(hosts);
        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let screen = screenshot::draw(&app, 80, 24);
        let top = card_row(&screen, list, "server-01");

        assert!(
            sidebar_row(&screen, list, top).contains(":2222"),
            "the port badge is missing:\n{}",
            screen
        );
        assert!(
            sidebar_row(&screen, list, top + 1).contains("id_ed25519"),
            "the key is missing:\n{}",
            screen
        );
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

        let list = super::frames(&app, Rect::new(0, 0, 80, 24)).list;
        let screen = screenshot::draw(&app, 80, 24);

        assert!(
            (list.y..list.bottom())
                .any(|row| sidebar_row(&screen, list, row).contains("server-40")),
            "selection scrolled out of view:\n{}",
            screen
        );
    }

    #[test]
    fn the_form_shows_every_field_on_a_small_terminal() {
        let (mut app, _repo) = app_with(hosts(3));
        app.begin_add();

        let screen = screenshot::draw(&app, 80, 24);

        for label in ["Host Alias", "HostName", "Port", "User", "IdentityFile"] {
            assert!(screen.contains(label), "form is missing '{}':\n{}", label, screen);
        }
        assert!(screen.contains("Tab next field"), "form is missing its footer:\n{}", screen);
        assert!(screen.contains(" Cancel "), "form is missing its way out:\n{}", screen);
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

        let screen = screenshot::draw(&app, 74, 20);

        assert!(screen.contains("closes this help"), "help is clipped:\n{}", screen);
        assert!(screen.contains("MOVE AROUND"), "help has lost its groups:\n{}", screen);
        assert!(
            screen.contains("show the ssh command"),
            "the last group is cut off:\n{}",
            screen
        );
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
    fn the_detail_panel_reads_like_the_config_file() {
        let (app, _repo) = app_with(hosts(3));

        let screen = screenshot::draw(&app, 100, 26);

        assert!(
            screen.contains(".ssh/config › server-01"),
            "the breadcrumb is missing:\n{}",
            screen
        );

        for (number, text) in [
            (1, "Host server-01"),
            (2, "HostName server-01.example.com"),
            (3, "Port 22"),
            (4, "User dperez"),
            (5, "IdentityFile (default)"),
        ] {
            assert!(
                screen.contains(&format!("{} │ {}", number, text)),
                "line {} should read '{}':\n{}",
                number,
                text,
                screen
            );
        }

        assert!(screen.contains("↵ connect"), "the panel lost its hints:\n{}", screen);
    }

    #[test]
    fn the_config_block_colours_the_keywords_by_where_they_came_from() {
        let mut list = hosts(2);
        list[0].extra_options = vec![("SetEnv".into(), "TERM=xterm-256color".into())];

        let (app, _repo) = app_with(list);
        let screen = screenshot::draw(&app, 100, 26);
        let buffer = screenshot::buffer(&app, 100, 26);
        let colour = |needle: &str| {
            let row = row_of(&screen, needle);
            buffer.get(column_of(&screen, row, needle), row).style().fg
        };

        assert_eq!(
            colour("Host server-01"),
            Some(app.theme.accent_secondary.to_color()),
            "the block header should stand apart:\n{}",
            screen
        );
        assert_eq!(
            colour("HostName"),
            Some(app.theme.accent.to_color()),
            "a field the form knows should wear the accent:\n{}",
            screen
        );
        assert_eq!(
            colour("SetEnv"),
            Some(app.theme.warning.to_color()),
            "a hand written option should be called out:\n{}",
            screen
        );
    }

    #[test]
    fn the_command_card_goes_away_without_taking_the_config_with_it() {
        let (mut app, _repo) = app_with(hosts(3));

        let shown = screenshot::draw(&app, 100, 26);
        assert!(shown.contains("$ ssh server-01"), "the command card is missing:\n{}", shown);

        app.toggle_command_preview();
        let hidden = screenshot::draw(&app, 100, 26);

        assert!(!hidden.contains("$ ssh server-01"), "the card should be gone:\n{}", hidden);
        assert!(
            hidden.contains("Host server-01"),
            "the config block should have stayed:\n{}",
            hidden
        );
    }

    #[test]
    fn the_command_card_says_what_the_probe_found() {
        let (up, _listener) = reachable();
        let (mut app, _repo) = app_with(vec![up, host("dead-host", 22)]);

        settle(|| !app.probes.is_working());

        let answered = screenshot::draw(&app, 100, 26);
        assert!(
            answered.contains("✓ reachable at dperez@127.0.0.1"),
            "a host that answered should say so:\n{}",
            answered
        );

        app.move_cursor_down();
        let silent = screenshot::draw(&app, 100, 26);
        assert!(
            silent.contains("✗ no answer from dead-host.example.com:22"),
            "a host that did not answer should say so:\n{}",
            silent
        );
    }

    #[test]
    fn the_status_bar_drops_its_tail_before_the_config_path() {
        let (app, _repo) = app_with(hosts(20));

        let wide = screenshot::draw(&app, 100, 18);
        let narrow = screenshot::draw(&app, 50, 18);

        assert!(wide.contains("stub"), "a wide bar should name the theme:\n{}", wide);
        assert!(
            narrow.lines().last().is_some_and(|bar| bar.contains(".ssh/config")),
            "the path should outlive the rest:\n{}",
            narrow
        );
    }
}





