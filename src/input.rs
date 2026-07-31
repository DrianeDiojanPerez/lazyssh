use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crossterm::terminal;
use ratatui::layout::Rect;

use crate::models::{Focus, Mode};
use crate::repositories::{SshRepository, ThemeRepository};
use crate::services::AppService;
use crate::ui::{panels, popups, renderer, tabs};

/// Waiting stops for a frame at a time while toasts are on screen so they can
/// animate, and blocks outright when there is nothing left to move.
const FRAME: Duration = Duration::from_millis(33);

/// Two clicks on the same spot inside this window count as a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// Remembers the last click, which is the only way to tell a double click from
/// two single ones.
#[derive(Default)]
pub struct Clicks {
    last: Option<(u16, u16, Instant)>,
}

impl Clicks {
    fn is_double(&mut self, column: u16, row: u16) -> bool {
        let now = Instant::now();
        let double = matches!(
            self.last,
            Some((c, r, at)) if c == column && r == row && now.duration_since(at) < DOUBLE_CLICK
        );

        // INFO: forgetting the click that completed a pair stops a third one
        // from connecting all over again
        self.last = (!double).then_some((column, row, now));
        double
    }
}

pub fn handle_next_event(
    app: &mut AppService,
    ssh_repo: &dyn SshRepository,
    theme_repo: &dyn ThemeRepository,
    clicks: &mut Clicks,
) -> std::io::Result<()> {
    // INFO: a live session paints without anyone touching the keyboard, so the
    // wait ends every frame while one is open
    if (app.has_toasts() || app.has_live_session() || app.probes.is_working())
        && !event::poll(FRAME)?
    {
        return Ok(());
    }

    match event::read()? {
        // INFO: a focused session owns the keyboard, so only the key that
        // brings the sidebar back is read here before the rest is forwarded
        Event::Key(key) if app.is_session_focused() && app.mode == Mode::Normal => {
            if is_sidebar_combo(key) {
                app.toggle_sidebar();
            } else if let Some(bytes) = encode(key) {
                if let Some(session) = app.active_session_mut() {
                    session.send(&bytes);
                }
            }
        }
        Event::Key(key) => dispatch_key(app, key, ssh_repo, theme_repo),
        Event::Mouse(mouse) => on_mouse(app, mouse, clicks, ssh_repo, theme_repo)?,
        _ => {}
    }
    Ok(())
}

/// Turns a key into the bytes a terminal would have sent for it.
fn encode(key: KeyEvent) -> Option<Vec<u8>> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let control = key.modifiers.contains(KeyModifiers::CONTROL);

    let bytes: Vec<u8> = match key.code {
        KeyCode::Char(c) if control => vec![(c.to_ascii_uppercase() as u8) & 0x1f],
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) if n <= 4 => vec![0x1b, b'O', b'P' + n - 1],
        _ => return None,
    };

    Some(match alt {
        true => [vec![0x1b], bytes].concat(),
        false => bytes,
    })
}

/// One way in for keys, whether they came from the keyboard or from a click on
/// the hint that names them.
fn dispatch_key(
    app: &mut AppService,
    key: KeyEvent,
    ssh_repo: &dyn SshRepository,
    theme_repo: &dyn ThemeRepository,
) {
    app.clear_form_error();

    if is_sidebar_combo(key) {
        app.toggle_sidebar();
        return;
    }

    match &app.mode {
        Mode::Normal => on_normal(app, key, ssh_repo, theme_repo),
        Mode::Search => on_search(app, key),
        Mode::AddHost => on_form(app, key, ssh_repo),
        Mode::EditHost(_) => on_form(app, key, ssh_repo),
        Mode::ConfirmDelete(idx) => on_confirm_delete(app, key, *idx, ssh_repo),
        Mode::SelectTheme => on_theme_select(app, key, theme_repo),
        Mode::Help => on_help(app, key),
    }
}

/// Everything on screen answers to the mouse: the wheel drives whichever list
/// is in front, and a click acts on the row, field or button underneath it.
fn on_mouse(
    app: &mut AppService,
    mouse: MouseEvent,
    clicks: &mut Clicks,
    ssh_repo: &dyn SshRepository,
    theme_repo: &dyn ThemeRepository,
) -> std::io::Result<()> {
    let (width, height) = terminal::size()?;
    let frames = renderer::frames(app, Rect::new(0, 0, width, height));
    let (column, row) = (mouse.column, mouse.row);

    match (&app.mode, mouse.kind) {
        (Mode::SelectTheme, MouseEventKind::ScrollUp) => return ok(app.theme_cursor_up()),
        (Mode::SelectTheme, MouseEventKind::ScrollDown) => return ok(app.theme_cursor_down()),

        (Mode::AddHost | Mode::EditHost(_), MouseEventKind::ScrollUp) => {
            return ok(app.suggestion_up())
        }
        (Mode::AddHost | Mode::EditHost(_), MouseEventKind::ScrollDown) => {
            return ok(app.suggestion_down())
        }

        (Mode::Normal | Mode::Search, MouseEventKind::ScrollUp) => {
            return ok(if hits(frames.list, column, row) {
                app.move_cursor_up()
            } else {
                app.scroll_session_back(3)
            })
        }
        (Mode::Normal | Mode::Search, MouseEventKind::ScrollDown) => {
            return ok(if hits(frames.list, column, row) {
                app.move_cursor_down()
            } else {
                app.scroll_session_back(-3)
            })
        }

        (_, MouseEventKind::Down(MouseButton::Left)) => {}
        _ => return Ok(()),
    }

    // INFO: the hints are a row of buttons in every mode, so they are offered
    // the click before whatever mode is on screen
    if let Some(code) = panels::hint_at(app, frames.status, column, row) {
        dispatch_key(app, KeyEvent::new(code, KeyModifiers::NONE), ssh_repo, theme_repo);
        return Ok(());
    }

    if let Some(area) = frames.tabs {
        match tabs::tab_at(app, area, column, row) {
            Some(tabs::TabHit::Select(index)) => return ok(app.select_tab(index)),
            Some(tabs::TabHit::Close(index)) => return ok(app.close_tab(index)),
            None => {}
        }
    }

    // clicking a pane is how the keyboard moves between them
    if app.mode == Mode::Normal && hits(frames.main, column, row) {
        return ok(app.focus_session());
    }
    if app.mode == Mode::Normal && app.focus == Focus::Session {
        app.focus_sidebar();
    }

    match app.mode.clone() {
        Mode::Normal | Mode::Search => {
            if hits(frames.search, column, row) {
                return ok(app.enter_search());
            }
            if let Some(index) = panels::host_at(app, frames.list, column, row) {
                app.select(index);
                if clicks.is_double(column, row) {
                    open_session(app);
                }
            }
        }

        Mode::AddHost | Mode::EditHost(_) => {
            let body = frames.body;

            if let Some(index) = popups::suggestion_at(app, body, column, row) {
                app.pick_suggestion(index);
            } else if let Some(button) = popups::form_button_at(app, body, column, row) {
                match button {
                    popups::FormButton::Save => commit_form(app, ssh_repo),
                    popups::FormButton::Cancel => app.cancel_mode(),
                }
            } else if let Some(field) = popups::field_at(app, body, column, row) {
                app.focus_field(field);
            }
        }

        Mode::ConfirmDelete(index) => match popups::delete_button_at(frames.body, column, row) {
            Some(popups::DeleteButton::Delete) => app.commit_delete(index, ssh_repo),
            Some(popups::DeleteButton::Keep) => app.cancel_mode(),
            None => {}
        },

        Mode::SelectTheme => {
            if let Some(index) = popups::theme_at(app, frames.body, column, row) {
                app.pick_theme(index, theme_repo);
            }
        }

        Mode::Help => app.cancel_mode(),
    }

    Ok(())
}

fn ok(_: ()) -> std::io::Result<()> {
    Ok(())
}

fn hits(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x && column < rect.right() && row >= rect.y && row < rect.bottom()
}

fn on_normal(
    app: &mut AppService,
    key: KeyEvent,
    ssh_repo: &dyn SshRepository,
    theme_repo: &dyn ThemeRepository,
) {
    if is_quit_combo(key) {
        app.request_quit();
        return;
    }

    match key.code {
        KeyCode::Char('q') => app.request_quit(),
        // INFO: an active filter is the first thing Esc should undo
        KeyCode::Esc if app.has_filter() => app.clear_filter(),
        KeyCode::Esc => app.request_quit(),

        KeyCode::Up | KeyCode::Char('k') => app.move_cursor_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_cursor_down(),
        KeyCode::Char('g') => app.jump_to_top(),
        KeyCode::Char('G') => app.jump_to_bottom(),

        KeyCode::Enter => open_session(app),
        KeyCode::Char('w') => app.close_active_tab(),
        KeyCode::Char('n') => app.next_tab(),
        KeyCode::Char('a') => app.begin_add(),
        KeyCode::Char('e') => app.begin_edit(),
        KeyCode::Char('d') => app.begin_delete(),
        KeyCode::Char('c') => app.toggle_command_preview(),
        KeyCode::Char('/') => app.enter_search(),
        KeyCode::Char('r') => app.reload_from_disk(ssh_repo),

        KeyCode::Char('t') => app.open_theme_selector(),
        KeyCode::Char('T') => app.toggle_transparency(theme_repo),
        KeyCode::Char('?') => app.open_help(),

        _ => {}
    }
}

/// The session is opened at the size of the pane it is about to be drawn in,
/// so the remote end never has to be told twice.
fn open_session(app: &mut AppService) {
    let Ok((width, height)) = terminal::size() else {
        return;
    };

    let pane = renderer::frames(app, Rect::new(0, 0, width, height)).main;
    app.open_session(pane.height.saturating_sub(2), pane.width.saturating_sub(2));
}

fn on_search(app: &mut AppService, key: KeyEvent) {
    if is_quit_combo(key) {
        app.request_quit();
        return;
    }

    match key.code {
        KeyCode::Esc => app.cancel_search(),
        KeyCode::Enter => app.finish_search(),
        KeyCode::Backspace => app.search_backspace(),
        KeyCode::Up => app.move_cursor_up(),
        KeyCode::Down => app.move_cursor_down(),
        KeyCode::Char(c) if !is_shortcut(key) => app.search_type(c),
        _ => {}
    }
}

fn on_form(app: &mut AppService, key: KeyEvent, ssh_repo: &dyn SshRepository) {
    if is_quit_combo(key) {
        app.request_quit();
        return;
    }

    match key.code {
        // INFO: the completion menu gets first refusal on the keys it needs,
        // so Esc closes the menu before it closes the form
        KeyCode::Down if app.is_completing() => app.suggestion_down(),
        KeyCode::Up if app.is_completing() => app.suggestion_up(),
        KeyCode::Esc if app.clear_suggestion() => {}

        KeyCode::Esc => app.cancel_mode(),

        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.form_previous_field();
            } else {
                app.form_next_field();
            }
        }
        KeyCode::BackTab => app.form_previous_field(),

        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            commit_form(app, ssh_repo);
        }
        KeyCode::Enter if app.accept_suggestion() => {}
        KeyCode::Enter => commit_form(app, ssh_repo),

        KeyCode::Backspace => app.form_delete_char(),
        KeyCode::Char(c) if !is_shortcut(key) => app.form_type_char(c),

        _ => {}
    }
}

fn commit_form(app: &mut AppService, ssh_repo: &dyn SshRepository) {
    match app.mode.clone() {
        Mode::AddHost => app.commit_add(ssh_repo),
        Mode::EditHost(idx) => app.commit_edit(idx, ssh_repo),
        _ => {}
    }
}

fn on_confirm_delete(
    app: &mut AppService,
    key: KeyEvent,
    index: usize,
    ssh_repo: &dyn SshRepository,
) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => app.commit_delete(index, ssh_repo),
        _ => app.cancel_mode(),
    }
}

fn on_theme_select(
    app: &mut AppService,
    key: KeyEvent,
    theme_repo: &dyn ThemeRepository,
) {
    match key.code {
        KeyCode::Esc => app.cancel_mode(),
        KeyCode::Up | KeyCode::Char('k') => app.theme_cursor_up(),
        KeyCode::Down | KeyCode::Char('j') => app.theme_cursor_down(),
        KeyCode::Enter => app.apply_selected_theme(theme_repo),
        _ => {}
    }
}

fn on_help(app: &mut AppService, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => app.cancel_mode(),
        _ => {}
    }
}

fn is_shortcut(key: KeyEvent) -> bool {
    key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Ctrl-B swings the sidebar in and out, and the keyboard follows it.
fn is_sidebar_combo(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_quit_combo(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_clicks_on_the_same_spot_are_a_double_click() {
        let mut clicks = Clicks::default();

        assert!(!clicks.is_double(10, 4), "the first click is never a double");
        assert!(clicks.is_double(10, 4));
    }

    #[test]
    fn a_click_somewhere_else_starts_over() {
        let mut clicks = Clicks::default();

        clicks.is_double(10, 4);
        assert!(!clicks.is_double(10, 9));
        assert!(clicks.is_double(10, 9));
    }

    #[test]
    fn a_third_click_does_not_count_twice() {
        let mut clicks = Clicks::default();

        clicks.is_double(10, 4);
        assert!(clicks.is_double(10, 4));
        assert!(!clicks.is_double(10, 4), "the pair should have been forgotten");
    }
}
