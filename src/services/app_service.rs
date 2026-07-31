use std::path::PathBuf;
use std::time::Duration;

use crate::models::{Action, Focus, FormField, Mode, SshHost, Theme, ThemePreference, Toast};
use crate::repositories::{SshRepository, ThemeRepository};
use crate::services::{Probes, Session};

const MAX_TOASTS: usize = 3;

pub struct AppService {
    preamble: String,
    hosts: Vec<SshHost>,
    ssh_config_path: PathBuf,

    pub mode: Mode,
    pub cursor: usize,
    pub form_draft: SshHost,
    pub form_field: FormField,
    // INFO: the port is kept as text while the form is open so it can be
    // cleared and retyped, and is only parsed when the form is saved
    form_port: String,

    pub theme: Theme,
    pub theme_preference: ThemePreference,
    pub available_themes: Vec<Theme>,
    pub theme_cursor: usize,

    pub search_query: String,
    pub visible_indices: Vec<usize>,
    pub show_command: bool,
    // INFO: the keys on disk are read once at startup and offered as
    // completions whenever the IdentityFile field has focus
    identity_files: Vec<String>,
    pub suggestion_cursor: Option<usize>,
    pub toasts: Vec<Toast>,
    // INFO: a rejected form stays open, so its complaint belongs inside the
    // form rather than in a toast that flies away
    pub form_error: Option<String>,
    pub pending_action: Action,

    // INFO: connections stay open in tabs, so the app is the terminal these
    // sessions live in rather than something that steps aside for them
    // INFO: who answered on their ssh port, kept beside the hosts so a card
    // can say at a glance whether it is worth pressing Enter on
    pub probes: Probes,
    pub sessions: Vec<Session>,
    pub active_tab: Option<usize>,
    pub focus: Focus,
    pub sidebar_open: bool,
}

impl AppService {
    pub fn initialize(
        ssh_repo: &dyn SshRepository,
        theme_repo: &dyn ThemeRepository,
    ) -> Self {
        let (preamble, hosts) = ssh_repo.load_all();
        let ssh_config_path = ssh_repo.config_path();

        let preference = theme_repo.load_preference();
        let available_themes = theme_repo.catalog();

        let theme_index = preference.theme_index.min(available_themes.len().saturating_sub(1));
        let mut theme = available_themes[theme_index].clone();
        theme.transparent = preference.transparent;

        let host_count = hosts.len();
        let probes = Probes::default();
        probes.check_all(&hosts);

        Self {
            preamble,
            hosts,
            ssh_config_path,

            mode: Mode::Normal,
            cursor: 0,
            form_draft: SshHost::empty(),
            form_field: FormField::Alias,
            form_port: String::new(),

            theme,
            theme_preference: preference,
            available_themes,
            theme_cursor: theme_index,

            search_query: String::new(),
            visible_indices: (0..host_count).collect(),
            show_command: true,
            identity_files: ssh_repo.identity_files(),
            suggestion_cursor: None,
            toasts: Vec::new(),
            form_error: None,
            pending_action: Action::Continue,

            probes,
            sessions: Vec::new(),
            active_tab: None,
            focus: Focus::Sidebar,
            sidebar_open: true,
        }
    }

    pub fn config_path_display(&self) -> String {
        let path = self.ssh_config_path.to_string_lossy().to_string();

        match dirs::home_dir() {
            Some(home) => path.replacen(&home.to_string_lossy().to_string(), "~", 1),
            None => path,
        }
    }

    pub fn host_count(&self) -> usize {
        self.hosts.len()
    }

    pub fn visible_hosts(&self) -> Vec<(usize, &SshHost)> {
        self.visible_indices
            .iter()
            .filter_map(|&i| self.hosts.get(i).map(|h| (i, h)))
            .collect()
    }

    pub fn selected_host(&self) -> Option<&SshHost> {
        self.visible_hosts().get(self.cursor).map(|(_, h)| *h)
    }

    fn selected_real_index(&self) -> Option<usize> {
        self.visible_hosts().get(self.cursor).map(|(i, _)| *i)
    }

    pub fn host_at(&self, index: usize) -> Option<&SshHost> {
        self.hosts.get(index)
    }

    // INFO: Navigation

    pub fn move_cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_down(&mut self) {
        let count = self.visible_hosts().len();
        if count > 0 && self.cursor < count - 1 {
            self.cursor += 1;
        }
    }

    /// Puts the cursor on a host by position in the visible list, which is what
    /// a click on a card asks for.
    pub fn select(&mut self, index: usize) {
        if index < self.visible_hosts().len() {
            self.cursor = index;
        }
    }

    pub fn jump_to_top(&mut self) {
        self.cursor = 0;
    }

    pub fn jump_to_bottom(&mut self) {
        let count = self.visible_hosts().len();
        if count > 0 {
            self.cursor = count - 1;
        }
    }

    // INFO: Search

    pub fn enter_search(&mut self) {
        self.search_query.clear();
        self.mode = Mode::Search;
    }

    pub fn search_type(&mut self, c: char) {
        self.search_query.push(c);
        self.rebuild_filter();
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.rebuild_filter();
    }

    pub fn finish_search(&mut self) {
        self.mode = Mode::Normal;
    }

    pub fn cancel_search(&mut self) {
        self.clear_filter();
        self.mode = Mode::Normal;
    }

    pub fn has_filter(&self) -> bool {
        !self.search_query.is_empty()
    }

    pub fn clear_filter(&mut self) {
        self.search_query.clear();
        self.rebuild_filter();
    }

    fn rebuild_filter(&mut self) {
        if self.search_query.is_empty() {
            self.visible_indices = (0..self.hosts.len()).collect();
        } else {
            let query = self.search_query.to_lowercase();
            self.visible_indices = self
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, h)| {
                    h.alias.to_lowercase().contains(&query)
                        || h.hostname.to_lowercase().contains(&query)
                        || h.user.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }

        let count = self.visible_indices.len();
        if self.cursor >= count {
            self.cursor = count.saturating_sub(1);
        }
    }

    // ─── CRUD via Repository ─────────────────────────────────────────────

    pub fn begin_add(&mut self) {
        self.suggestion_cursor = None;
        self.form_draft = SshHost::empty();
        self.form_port = String::new();
        self.form_field = FormField::Alias;
        self.mode = Mode::AddHost;
    }

    pub fn begin_edit(&mut self) {
        self.suggestion_cursor = None;
        if let Some(index) = self.selected_real_index() {
            self.form_draft = self.hosts[index].clone();
            self.form_port = self.form_draft.port.to_string();
            self.form_field = FormField::Alias;
            self.mode = Mode::EditHost(index);
        }
    }

    pub fn begin_delete(&mut self) {
        if let Some(index) = self.selected_real_index() {
            self.mode = Mode::ConfirmDelete(index);
        }
    }

    fn build_draft(&self) -> Result<SshHost, String> {
        let mut draft = SshHost {
            alias: self.form_draft.alias.trim().to_string(),
            hostname: self.form_draft.hostname.trim().to_string(),
            port: SshHost::DEFAULT_PORT,
            user: self.form_draft.user.trim().to_string(),
            identity_file: self.form_draft.identity_file.trim().to_string(),
            extra_options: self.form_draft.extra_options.clone(),
        };

        if !draft.is_valid() {
            return Err("Alias and HostName are required".into());
        }
        if draft.alias.split_whitespace().count() > 1 {
            return Err("Alias cannot contain spaces".into());
        }
        if draft.hostname.split_whitespace().count() > 1 {
            return Err("HostName cannot contain spaces".into());
        }

        draft.port = self.parse_form_port()?;
        Ok(draft)
    }

    pub fn commit_add(&mut self, ssh_repo: &dyn SshRepository) {
        let draft = match self.build_draft() {
            Ok(draft) => draft,
            Err(message) => {
                self.form_error = Some(message);
                return;
            }
        };

        let alias_exists = self.hosts.iter().any(|h| {
            h.alias.to_lowercase() == draft.alias.to_lowercase()
        });
        if alias_exists {
            self.form_error = Some(format!("'{}' already exists", draft.alias));
            return;
        }

        let name = draft.alias.clone();
        self.hosts.push(draft);

        match ssh_repo.save_all(&self.preamble, &self.hosts) {
            Ok(_) => self.toast(Toast::success(format!("Added '{}'", name))),
            Err(e) => {
                self.hosts.pop();
                self.toast(Toast::error(e));
            }
        }

        self.rebuild_filter();
        self.mode = Mode::Normal;
    }

    pub fn commit_edit(&mut self, index: usize, ssh_repo: &dyn SshRepository) {
        let draft = match self.build_draft() {
            Ok(draft) => draft,
            Err(message) => {
                self.form_error = Some(message);
                return;
            }
        };

        let duplicate = self.hosts.iter().enumerate().any(|(i, h)| {
            i != index && h.alias.to_lowercase() == draft.alias.to_lowercase()
        });
        if duplicate {
            self.form_error = Some(format!("'{}' already exists", draft.alias));
            return;
        }

        let name = draft.alias.clone();
        let backup = self.hosts[index].clone();
        self.hosts[index] = draft;

        match ssh_repo.save_all(&self.preamble, &self.hosts) {
            Ok(_) => self.toast(Toast::success(format!("Updated '{}'", name))),
            Err(e) => {
                self.hosts[index] = backup;
                self.toast(Toast::error(e));
            }
        }

        self.rebuild_filter();
        self.mode = Mode::Normal;
    }

    pub fn commit_delete(&mut self, index: usize, ssh_repo: &dyn SshRepository) {
        let removed = self.hosts.remove(index);

        match ssh_repo.save_all(&self.preamble, &self.hosts) {
            Ok(_) => self.toast(Toast::success(format!("Deleted '{}'", removed.alias))),
            Err(e) => {
                self.hosts.insert(index, removed);
                self.toast(Toast::error(e));
            }
        }

        self.rebuild_filter();
        let count = self.visible_hosts().len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        }
        self.mode = Mode::Normal;
    }

    pub fn cancel_mode(&mut self) {
        self.mode = Mode::Normal;
    }

    // ─── SSH Execution ───────────────────────────────────────────────────

    /// Opens the selected host in a tab of its own and hands it the keyboard.
    /// A host already open is brought forward instead of dialled twice.
    pub fn open_session(&mut self, rows: u16, columns: u16) {
        let Some(host) = self.selected_host() else {
            return;
        };

        let alias = host.alias.clone();
        if let Some(index) = self.sessions.iter().position(|s| s.alias == alias) {
            self.select_tab(index);
            return;
        }

        match Session::open(&alias, &host.as_ssh_args(), rows, columns) {
            Ok(session) => {
                self.sessions.push(session);
                self.select_tab(self.sessions.len() - 1);
                self.toast(Toast::success(format!("Connected to '{}'", alias)));
            }
            Err(message) => self.toast(Toast::error(message)),
        }
    }

    pub fn select_tab(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active_tab = Some(index);
            self.focus = Focus::Session;
        }
    }

    pub fn close_tab(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }

        self.sessions.remove(index);
        self.active_tab = match self.sessions.len() {
            0 => None,
            len => Some(self.active_tab.unwrap_or(0).min(len - 1)),
        };

        if self.active_tab.is_none() {
            self.focus = Focus::Sidebar;
        }
    }

    pub fn close_active_tab(&mut self) {
        if let Some(index) = self.active_tab {
            self.close_tab(index);
        }
    }

    pub fn next_tab(&mut self) {
        if let Some(index) = self.active_tab {
            self.select_tab((index + 1) % self.sessions.len());
        }
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.sessions.get(self.active_tab?)
    }

    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(self.active_tab?)
    }

    pub fn has_live_session(&self) -> bool {
        self.sessions.iter().any(|session| session.is_running())
    }

    /// Closing the sidebar hands the keyboard to the session, and opening it
    /// takes it back, so one key does the whole move.
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_open = !self.sidebar_open;
        self.focus = if self.sidebar_open || self.active_tab.is_none() {
            Focus::Sidebar
        } else {
            Focus::Session
        };
    }

    pub fn focus_sidebar(&mut self) {
        self.focus = Focus::Sidebar;
        if !self.sidebar_open {
            self.sidebar_open = true;
        }
    }

    pub fn focus_session(&mut self) {
        if self.active_tab.is_some() {
            self.focus = Focus::Session;
        }
    }

    /// Walks the active session back through its scrollback, for the wheel.
    pub fn scroll_session_back(&mut self, lines: i32) {
        let Some(session) = self.active_session() else {
            return;
        };

        let Ok(mut parser) = session.screen.lock() else {
            return;
        };

        let current = parser.screen().scrollback() as i32;
        parser.screen_mut().set_scrollback((current + lines).max(0) as usize);
    }

    pub fn is_session_focused(&self) -> bool {
        self.focus == Focus::Session && self.active_tab.is_some()
    }

    // ─── Form Editing ────────────────────────────────────────────────────

    // ─── IdentityFile completion ─────────────────────────────────────────

    /// The keys worth offering for what has been typed so far. Everything is
    /// on the table until the field says otherwise.
    pub fn identity_matches(&self) -> Vec<&str> {
        let typed = self.form_draft.identity_file.trim().to_lowercase();

        self.identity_files
            .iter()
            .filter(|path| typed.is_empty() || path.to_lowercase().contains(&typed))
            .map(|path| path.as_str())
            .collect()
    }

    pub fn is_completing(&self) -> bool {
        self.form_field == FormField::IdentityFile && !self.identity_matches().is_empty()
    }

    pub fn suggestion_down(&mut self) {
        let count = self.identity_matches().len();
        if count == 0 {
            return;
        }

        self.suggestion_cursor = Some(match self.suggestion_cursor {
            Some(current) => (current + 1) % count,
            None => 0,
        });
    }

    pub fn suggestion_up(&mut self) {
        let count = self.identity_matches().len();
        if count == 0 {
            return;
        }

        self.suggestion_cursor = Some(match self.suggestion_cursor {
            Some(0) | None => count - 1,
            Some(current) => current - 1,
        });
    }

    /// Puts the highlighted key into the field. False when nothing was
    /// highlighted, which leaves the keypress to whoever wants it next.
    pub fn accept_suggestion(&mut self) -> bool {
        let Some(index) = self.suggestion_cursor else {
            return false;
        };

        let Some(path) = self.identity_matches().get(index).map(|p| p.to_string()) else {
            return false;
        };

        self.form_draft.identity_file = path;
        self.suggestion_cursor = None;
        true
    }

    /// Takes the key a click landed on, wherever the highlight happened to be.
    pub fn pick_suggestion(&mut self, index: usize) {
        self.suggestion_cursor = Some(index);
        self.accept_suggestion();
    }

    pub fn clear_suggestion(&mut self) -> bool {
        self.suggestion_cursor.take().is_some()
    }

    /// Puts the cursor in a field, for a click straight onto one.
    pub fn focus_field(&mut self, field: FormField) {
        self.suggestion_cursor = None;
        self.form_field = field;
    }

    pub fn form_next_field(&mut self) {
        self.suggestion_cursor = None;
        self.form_field = self.form_field.next();
    }

    pub fn form_previous_field(&mut self) {
        self.suggestion_cursor = None;
        self.form_field = self.form_field.previous();
    }

    pub fn form_type_char(&mut self, c: char) {
        self.suggestion_cursor = None;
        if !self.form_field.accepts_char(c) {
            return;
        }
        let mut value = self.read_form_field();
        value.push(c);
        self.write_form_field(value);
    }

    pub fn form_delete_char(&mut self) {
        self.suggestion_cursor = None;
        let mut value = self.read_form_field();
        value.pop();
        self.write_form_field(value);
    }

    pub fn form_value(&self, field: &FormField) -> String {
        match field {
            FormField::Alias => self.form_draft.alias.clone(),
            FormField::HostName => self.form_draft.hostname.clone(),
            FormField::Port => self.form_port.clone(),
            FormField::User => self.form_draft.user.clone(),
            FormField::IdentityFile => self.form_draft.identity_file.clone(),
        }
    }

    fn read_form_field(&self) -> String {
        self.form_value(&self.form_field)
    }

    fn write_form_field(&mut self, value: String) {
        match self.form_field {
            FormField::Alias => self.form_draft.alias = value,
            FormField::HostName => self.form_draft.hostname = value,
            FormField::Port => self.form_port = value,
            FormField::User => self.form_draft.user = value,
            FormField::IdentityFile => self.form_draft.identity_file = value,
        }
    }

    fn parse_form_port(&self) -> Result<u16, String> {
        match self.form_port.trim() {
            "" => Ok(SshHost::DEFAULT_PORT),
            text => text
                .parse::<u16>()
                .ok()
                .filter(|port| *port > 0)
                .ok_or_else(|| "Port must be between 1 and 65535".to_string()),
        }
    }

    // ─── Theme ───────────────────────────────────────────────────────────

    pub fn open_theme_selector(&mut self) {
        self.theme_cursor = self.theme_preference.theme_index;
        self.mode = Mode::SelectTheme;
    }

    pub fn theme_cursor_up(&mut self) {
        if self.theme_cursor > 0 {
            self.theme_cursor -= 1;
        }
    }

    pub fn theme_cursor_down(&mut self) {
        if self.theme_cursor < self.available_themes.len() - 1 {
            self.theme_cursor += 1;
        }
    }

    pub fn apply_selected_theme(&mut self, theme_repo: &dyn ThemeRepository) {
        let index = self.theme_cursor;
        if let Some(new_theme) = self.available_themes.get(index) {
            let mut theme = new_theme.clone();
            if index == 0 {
                theme.transparent = true;
            } else {
                theme.transparent = self.theme_preference.transparent;
            }

            self.theme = theme;
            self.theme_preference.theme_index = index;
            self.theme_preference.transparent = self.theme.transparent;
            theme_repo.save_preference(&self.theme_preference);

            let name = new_theme.name.clone();
            self.toast(Toast::success(format!("Theme: {}", name)));
        }
        self.mode = Mode::Normal;
    }

    pub fn pick_theme(&mut self, index: usize, theme_repo: &dyn ThemeRepository) {
        self.theme_cursor = index;
        self.apply_selected_theme(theme_repo);
    }

    pub fn toggle_transparency(&mut self, theme_repo: &dyn ThemeRepository) {
        self.theme.transparent = !self.theme.transparent;
        self.theme_preference.transparent = self.theme.transparent;
        theme_repo.save_preference(&self.theme_preference);

        let label = if self.theme.transparent { "ON" } else { "OFF" };
        self.toast(Toast::success(format!("Transparency: {}", label)));
    }

    // ─── Misc ────────────────────────────────────────────────────────────

    pub fn toggle_command_preview(&mut self) {
        self.show_command = !self.show_command;
    }

    pub fn reload_from_disk(&mut self, ssh_repo: &dyn SshRepository) {
        let (preamble, hosts) = ssh_repo.load_all();
        self.preamble = preamble;
        self.hosts = hosts;
        self.rebuild_filter();
        self.probes.check_all(&self.hosts);
        self.toast(Toast::success(format!("Reloaded ({} hosts)", self.hosts.len())));
    }

    pub fn open_help(&mut self) {
        self.mode = Mode::Help;
    }

    pub fn request_quit(&mut self) {
        self.pending_action = Action::Quit;
    }

    pub fn clear_form_error(&mut self) {
        self.form_error = None;
    }

    fn toast(&mut self, toast: Toast) {
        // INFO: a corner full of toasts hides the app, so the oldest gives way
        if self.toasts.len() == MAX_TOASTS {
            self.toasts.remove(0);
        }
        self.toasts.push(toast);
    }

    /// Ages every toast by the time the last frame took and drops the ones
    /// that have finished, which is what makes them animate.
    pub fn advance_toasts(&mut self, delta: Duration) {
        for toast in &mut self.toasts {
            toast.advance(delta);
        }
        self.toasts.retain(|toast| !toast.is_finished());
    }

    pub fn has_toasts(&self) -> bool {
        !self.toasts.is_empty()
    }

    pub fn take_action(&mut self) -> Action {
        let action = self.pending_action.clone();
        self.pending_action = Action::Continue;
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{app_with, host};

    fn type_into(app: &mut AppService, field: FormField, text: &str) {
        app.form_field = field;
        for c in text.chars() {
            app.form_type_char(c);
        }
    }

    #[test]
    fn typing_a_port_replaces_the_default_instead_of_appending() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "box");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        type_into(&mut app, FormField::Port, "2222");
        app.commit_add(&repo);

        assert_eq!(app.host_at(0).map(|h| h.port), Some(2222));
    }

    #[test]
    fn port_field_can_be_cleared_and_retyped() {
        let (mut app, repo) = app_with(vec![host("box", 2222)]);

        app.begin_edit();
        app.form_field = FormField::Port;
        for _ in 0..4 {
            app.form_delete_char();
        }
        assert_eq!(app.form_value(&FormField::Port), "");

        for c in "8022".chars() {
            app.form_type_char(c);
        }
        app.commit_edit(0, &repo);

        assert_eq!(app.host_at(0).map(|h| h.port), Some(8022));
    }

    #[test]
    fn an_empty_port_falls_back_to_22() {
        let (mut app, repo) = app_with(vec![host("box", 2222)]);

        app.begin_edit();
        app.form_field = FormField::Port;
        for _ in 0..4 {
            app.form_delete_char();
        }
        app.commit_edit(0, &repo);

        assert_eq!(app.host_at(0).map(|h| h.port), Some(22));
    }

    #[test]
    fn an_out_of_range_port_is_rejected_instead_of_silently_reset() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "box");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        type_into(&mut app, FormField::Port, "999999");
        app.commit_add(&repo);

        assert_eq!(app.host_count(), 0);
        assert_eq!(app.mode, Mode::AddHost);
        assert!(app.form_error.is_some());
    }

    #[test]
    fn editing_shows_the_current_port() {
        let (mut app, _repo) = app_with(vec![host("box", 2222)]);

        app.begin_edit();

        assert_eq!(app.form_value(&FormField::Port), "2222");
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_before_saving() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "  box  ");
        type_into(&mut app, FormField::HostName, " 10.0.0.5 ");
        type_into(&mut app, FormField::User, " root ");
        app.commit_add(&repo);

        let saved = app.host_at(0).expect("host was saved");
        assert_eq!(saved.alias, "box");
        assert_eq!(saved.hostname, "10.0.0.5");
        assert_eq!(saved.user, "root");
    }

    #[test]
    fn an_alias_with_an_inner_space_is_rejected() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "my box");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        app.commit_add(&repo);

        assert_eq!(app.host_count(), 0);
        assert_eq!(app.mode, Mode::AddHost);
    }

    #[test]
    fn the_key_list_narrows_to_what_has_been_typed() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.begin_add();

        assert_eq!(app.identity_matches().len(), 3, "every key is on offer to begin with");

        type_into(&mut app, FormField::IdentityFile, "work");
        assert_eq!(app.identity_matches(), vec!["~/.ssh/work_ed25519"]);
    }

    #[test]
    fn picking_a_key_fills_the_field_in() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.begin_add();
        app.form_field = FormField::IdentityFile;

        assert!(!app.accept_suggestion(), "nothing is highlighted yet");

        app.suggestion_down();
        app.suggestion_down();
        assert!(app.accept_suggestion());
        assert_eq!(app.form_draft.identity_file, "~/.ssh/id_rsa");
        assert!(app.suggestion_cursor.is_none(), "the menu closes once a key is taken");
    }

    #[test]
    fn the_menu_only_belongs_to_the_identity_field() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.begin_add();

        assert!(!app.is_completing());

        app.form_field = FormField::IdentityFile;
        assert!(app.is_completing());
    }

    #[test]
    fn a_whitespace_only_alias_is_rejected() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "   ");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        app.commit_add(&repo);

        assert_eq!(app.host_count(), 0);
        assert_eq!(app.mode, Mode::AddHost);
    }

    #[test]
    fn a_duplicate_alias_is_rejected_after_trimming() {
        let (mut app, repo) = app_with(vec![host("box", 22)]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, " BOX ");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        app.commit_add(&repo);

        assert_eq!(app.host_count(), 1);
        assert_eq!(app.mode, Mode::AddHost);
    }
}
