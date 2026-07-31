use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crossterm::terminal;
use ratatui::layout::Rect;

use crate::models::Mode;
use crate::repositories::{SshRepository, ThemeRepository};
use crate::services::AppService;
use crate::ui::{panels, renderer};

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
    if app.has_toasts() && !event::poll(FRAME)? {
        return Ok(());
    }

    match event::read()? {
        Event::Key(key) => {
            app.clear_form_error();

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
        Event::Mouse(mouse) => on_mouse(app, mouse, clicks)?,
        _ => {}
    }
    Ok(())
}

/// The wheel moves whichever list is in front, and a click picks the host it
/// lands on. Popups keep the mouse out so nothing is confirmed by accident.
fn on_mouse(app: &mut AppService, mouse: MouseEvent, clicks: &mut Clicks) -> std::io::Result<()> {
    match (&app.mode, mouse.kind) {
        (Mode::SelectTheme, MouseEventKind::ScrollUp) => app.theme_cursor_up(),
        (Mode::SelectTheme, MouseEventKind::ScrollDown) => app.theme_cursor_down(),

        (Mode::Normal | Mode::Search, MouseEventKind::ScrollUp) => app.move_cursor_up(),
        (Mode::Normal | Mode::Search, MouseEventKind::ScrollDown) => app.move_cursor_down(),

        (Mode::Normal | Mode::Search, MouseEventKind::Down(MouseButton::Left)) => {
            let (width, height) = terminal::size()?;
            let list = renderer::frames(Rect::new(0, 0, width, height), app.mode == Mode::Search).list;

            if let Some(index) = panels::host_at(app, list, mouse.column, mouse.row) {
                app.select(index);
                if clicks.is_double(mouse.column, mouse.row) {
                    app.launch_ssh();
                }
            }
        }

        _ => {}
    }
    Ok(())
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

        KeyCode::Enter => app.launch_ssh(),
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
