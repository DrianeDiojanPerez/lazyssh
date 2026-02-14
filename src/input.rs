use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::models::Mode;
use crate::repositories::{SshRepository, ThemeRepository};
use crate::services::AppService;

pub fn handle_next_event(
    app: &mut AppService,
    ssh_repo: &dyn SshRepository,
    theme_repo: &dyn ThemeRepository,
) -> std::io::Result<()> {
    if let Event::Key(key) = event::read()? {
        app.clear_notification();

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
        KeyCode::Char('q') | KeyCode::Esc => app.request_quit(),

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
        KeyCode::Char(c) => app.search_type(c),
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
        KeyCode::Char(c) => app.form_type_char(c),

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

fn is_quit_combo(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}
