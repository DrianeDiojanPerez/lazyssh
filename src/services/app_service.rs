use std::path::PathBuf;
use std::time::Duration;

use crate::models::{
    Action, Focus, FormField, LaunchStyle, Mode, Setting, SshHost, Theme, ThemePreference, Toast,
};
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
    form_options: String,

    pub theme: Theme,
    pub theme_preference: ThemePreference,
    pub available_themes: Vec<Theme>,
    pub theme_cursor: usize,

    pub search_query: String,
    pub visible_indices: Vec<usize>,
    pub show_command: bool,
    identity_files: Vec<String>,
    pub suggestion_cursor: Option<usize>,
    pub toasts: Vec<Toast>,
    pub form_error: Option<String>,
    pub pending_action: Action,

    pub probes: Probes,
    pub sessions: Vec<Session>,
    pub active_tab: Option<usize>,
    pub focus: Focus,
    pub sidebar_open: bool,
    pub settings_cursor: usize,
    pub launch_cursor: usize,
    theme_from_settings: bool,
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
        // the first theme in the catalog is the one that lets the terminal
        // show through, whatever the preference says
        theme.transparent = theme_index == 0 || preference.transparent;

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
            form_options: String::new(),

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
            settings_cursor: 0,
            launch_cursor: 0,
            theme_from_settings: false,
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

    pub fn begin_add(&mut self) {
        self.suggestion_cursor = None;
        self.form_draft = SshHost::empty();
        self.form_port = String::new();
        self.form_options = String::new();
        self.form_field = FormField::Alias;
        self.mode = Mode::AddHost;
    }

    pub fn begin_edit(&mut self) {
        self.suggestion_cursor = None;
        if let Some(index) = self.selected_real_index() {
            self.form_draft = self.hosts[index].clone();
            self.form_port = self.form_draft.port.to_string();
            self.form_options = write_options(&self.form_draft.extra_options);
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
            extra_options: Vec::new(),
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
        draft.extra_options = parse_options(&self.form_options)?;
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
        if self.mode == Mode::SelectTheme {
            self.restore_theme();
        }
        self.mode = self.mode_behind();
    }

    /// Where Esc lands: back in the settings panel when that is what opened
    /// the popup, and on the host list otherwise.
    fn mode_behind(&mut self) -> Mode {
        match self.mode == Mode::SelectTheme && self.theme_from_settings {
            true => {
                self.theme_from_settings = false;
                Mode::Settings
            }
            false => Mode::Normal,
        }
    }

    pub fn request_connection(&mut self, rows: u16, columns: u16) {
        if self.selected_host().is_none() {
            return;
        }

        match self.theme_preference.launch_style {
            LaunchStyle::Ask => {
                self.launch_cursor = 0;
                self.mode = Mode::ChooseLaunch;
            }
            LaunchStyle::Tab => self.open_session(rows, columns),
            LaunchStyle::FullScreen => self.launch_full_screen(),
        }
    }

    pub fn launch_cursor_left(&mut self) {
        self.launch_cursor = 0;
    }

    pub fn launch_cursor_right(&mut self) {
        self.launch_cursor = 1;
    }

    pub fn launch_full_screen(&mut self) {
        if let Some(host) = self.selected_host() {
            self.pending_action = Action::LaunchSsh(host.as_ssh_args());
        }
        self.mode = Mode::Normal;
    }

    pub fn open_settings(&mut self) {
        self.settings_cursor = Setting::all()
            .iter()
            .position(|row| *row == Setting::Launch(self.theme_preference.launch_style))
            .unwrap_or(0);
        self.mode = Mode::Settings;
    }

    pub fn settings_cursor_up(&mut self) {
        self.settings_cursor = self.settings_cursor.saturating_sub(1);
    }

    pub fn settings_cursor_down(&mut self) {
        self.settings_cursor = (self.settings_cursor + 1).min(Setting::all().len() - 1);
    }

    pub fn apply_settings_choice(&mut self, theme_repo: &dyn ThemeRepository) {
        match Setting::all().get(self.settings_cursor) {
            Some(Setting::Launch(style)) => self.choose_launch_style(*style, theme_repo),
            Some(Setting::Transparency) => self.toggle_transparency(theme_repo),
            Some(Setting::TabEdges) => self.toggle_tab_edges(theme_repo),
            Some(Setting::TabPanel) => self.toggle_tab_panel(theme_repo),
            Some(Setting::Theme) => {
                // INFO: the picker is opened from here, so Esc out of it comes
                // back here rather than dropping the whole panel
                self.theme_from_settings = true;
                self.open_theme_selector();
            }
            None => {}
        }
    }

    pub fn choose_launch_style(&mut self, style: LaunchStyle, theme_repo: &dyn ThemeRepository) {
        self.theme_preference.launch_style = style;
        theme_repo.save_preference(&self.theme_preference);
        self.toast(Toast::success(format!("Connections: {}", style.label().to_lowercase())));
    }

    pub fn launch_style(&self) -> LaunchStyle {
        self.theme_preference.launch_style
    }

    pub fn open_session(&mut self, rows: u16, columns: u16) {
        let Some(host) = self.selected_host() else {
            return;
        };

        let alias = host.alias.clone();
        let args = host.as_ssh_args();
        self.mode = Mode::Normal;
        if let Some(index) = self.sessions.iter().position(|s| s.alias == alias) {
            self.select_tab(index);
            return;
        }

        match Session::open(&alias, &args, rows, columns) {
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

    /// Closes the tabs that have nothing left to say. A session that ran and
    /// ended goes away on its own, and one ssh never got going stays put so
    /// the reason it printed can still be read.
    pub fn reap_finished_sessions(&mut self) {
        let mut closed = Vec::new();
        let mut index = 0;

        while index < self.sessions.len() {
            if self.sessions[index].is_finished() {
                closed.push(self.sessions[index].alias.clone());
                self.close_tab(index);
            } else {
                index += 1;
            }
        }

        for alias in closed {
            self.toast(Toast::success(format!("'{}' disconnected", alias)));
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

    /// INFO: a popup takes the keyboard from the session while it is open, so
    /// the session only counts as focused when nothing is in front of it
    pub fn is_session_focused(&self) -> bool {
        self.mode == Mode::Normal && self.focus == Focus::Session && self.active_tab.is_some()
    }

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

    pub fn pick_suggestion(&mut self, index: usize) {
        self.suggestion_cursor = Some(index);
        self.accept_suggestion();
    }

    pub fn clear_suggestion(&mut self) -> bool {
        self.suggestion_cursor.take().is_some()
    }

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

    pub fn form_new_line(&mut self) {
        self.suggestion_cursor = None;
        let mut value = self.read_form_field();
        value.push('\n');
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
            FormField::Options => self.form_options.clone(),
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
            FormField::Options => self.form_options = value,
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

    pub fn open_theme_selector(&mut self) {
        self.theme_cursor = self.theme_preference.theme_index;
        self.mode = Mode::SelectTheme;
    }

    pub fn theme_cursor_up(&mut self) {
        if self.theme_cursor > 0 {
            self.theme_cursor -= 1;
            self.preview_theme();
        }
    }

    pub fn theme_cursor_down(&mut self) {
        if self.theme_cursor < self.available_themes.len() - 1 {
            self.theme_cursor += 1;
            self.preview_theme();
        }
    }

    fn preview_theme(&mut self) {
        if let Some(theme) = self.theme_at(self.theme_cursor) {
            self.theme = theme;
        }
    }

    fn theme_at(&self, index: usize) -> Option<Theme> {
        let mut theme = self.available_themes.get(index)?.clone();
        theme.transparent = index == 0 || self.theme_preference.transparent;
        Some(theme)
    }

    fn restore_theme(&mut self) {
        if let Some(theme) = self.theme_at(self.theme_preference.theme_index) {
            self.theme = theme;
        }
    }

    pub fn apply_selected_theme(&mut self, theme_repo: &dyn ThemeRepository) {
        let index = self.theme_cursor;
        if let Some(theme) = self.theme_at(index) {
            let name = theme.name.clone();

            self.theme = theme;
            self.theme_preference.theme_index = index;
            self.theme_preference.transparent = self.theme.transparent;
            theme_repo.save_preference(&self.theme_preference);

            self.toast(Toast::success(format!("Theme: {}", name)));
        }
        self.mode = self.mode_behind();
    }

    pub fn pick_theme(&mut self, index: usize, theme_repo: &dyn ThemeRepository) {
        self.theme_cursor = index;
        self.apply_selected_theme(theme_repo);
    }

    pub fn tab_edges(&self) -> bool {
        self.theme_preference.tab_edges
    }

    pub fn toggle_tab_edges(&mut self, theme_repo: &dyn ThemeRepository) {
        self.theme_preference.tab_edges = !self.theme_preference.tab_edges;
        theme_repo.save_preference(&self.theme_preference);

        let label = if self.tab_edges() { "on" } else { "off" };
        self.toast(Toast::success(format!("Slanted tabs: {}", label)));
    }

    pub fn tab_panel(&self) -> bool {
        self.theme_preference.tab_panel
    }

    pub fn toggle_tab_panel(&mut self, theme_repo: &dyn ThemeRepository) {
        self.theme_preference.tab_panel = !self.theme_preference.tab_panel;
        theme_repo.save_preference(&self.theme_preference);

        let label = if self.tab_panel() { "on" } else { "off" };
        self.toast(Toast::success(format!("Tabs in a panel: {}", label)));
    }

    pub fn toggle_transparency(&mut self, theme_repo: &dyn ThemeRepository) {
        self.theme.transparent = !self.theme.transparent;
        self.theme_preference.transparent = self.theme.transparent;
        theme_repo.save_preference(&self.theme_preference);

        let label = if self.theme.transparent { "ON" } else { "OFF" };
        self.toast(Toast::success(format!("Transparency: {}", label)));
    }

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

/// The options the form has no field of its own for, written the way they read
/// in the file: a name, a space and its value, one to a line.
fn write_options(options: &[(String, String)]) -> String {
    options
        .iter()
        .map(|(name, value)| format!("{} {}", name, value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_options(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut parsed = Vec::new();

    for entry in text.lines() {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        match entry.split_once(char::is_whitespace) {
            Some((name, value)) if !value.trim().is_empty() => {
                parsed.push((name.to_string(), value.trim().to_string()))
            }
            _ => return Err(format!("'{}' needs a name and a value", entry)),
        }
    }

    Ok(parsed)
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
    fn an_option_the_form_has_no_field_for_can_still_be_typed_in() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "box");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        type_into(&mut app, FormField::Options, "SetEnv TERM=xterm-256color");
        app.commit_add(&repo);

        assert_eq!(
            app.host_at(0).map(|h| h.extra_options.clone()),
            Some(vec![("SetEnv".into(), "TERM=xterm-256color".into())])
        );
    }

    #[test]
    fn the_options_a_host_already_had_come_back_up_for_editing() {
        let mut box_host = host("box", 22);
        box_host.extra_options = vec![
            ("HostKeyAlgorithms".into(), "ssh-rsa".into()),
            ("SetEnv".into(), "TERM=xterm-256color".into()),
        ];

        let (mut app, repo) = app_with(vec![box_host]);
        app.begin_edit();

        assert_eq!(
            app.form_value(&FormField::Options),
            "HostKeyAlgorithms ssh-rsa\nSetEnv TERM=xterm-256color"
        );

        app.form_field = FormField::Options;
        app.form_new_line();
        type_into(&mut app, FormField::Options, "Compression yes");
        app.commit_edit(0, &repo);

        assert_eq!(
            app.host_at(0).map(|h| h.extra_options.len()),
            Some(3),
            "the option typed on a new line should have been kept"
        );
    }

    #[test]
    fn a_semicolon_in_an_option_stays_part_of_its_value() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "box");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        type_into(&mut app, FormField::Options, "ProxyCommand ssh -W %h:%p jump; true");
        app.commit_add(&repo);

        assert_eq!(
            app.host_at(0).map(|h| h.extra_options.clone()),
            Some(vec![("ProxyCommand".into(), "ssh -W %h:%p jump; true".into())])
        );
    }

    #[test]
    fn an_option_with_nothing_but_a_name_is_refused() {
        let (mut app, repo) = app_with(vec![]);

        app.begin_add();
        type_into(&mut app, FormField::Alias, "box");
        type_into(&mut app, FormField::HostName, "10.0.0.5");
        type_into(&mut app, FormField::Options, "Compression");
        app.commit_add(&repo);

        assert_eq!(app.host_count(), 0, "the host should not have been saved");
        assert_eq!(
            app.form_error.as_deref(),
            Some("'Compression' needs a name and a value")
        );
    }

    #[test]
    fn clearing_the_options_field_takes_the_options_off_the_host() {
        let mut box_host = host("box", 22);
        box_host.extra_options = vec![("Compression".into(), "yes".into())];

        let (mut app, repo) = app_with(vec![box_host]);
        app.begin_edit();
        app.form_field = FormField::Options;
        while !app.form_value(&FormField::Options).is_empty() {
            app.form_delete_char();
        }
        app.commit_edit(0, &repo);

        assert_eq!(app.host_at(0).map(|h| h.extra_options.is_empty()), Some(true));
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
    fn the_old_way_hands_the_whole_terminal_over() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.choose_launch_style(LaunchStyle::FullScreen, &crate::test_support::StubThemeRepo);

        app.request_connection(20, 40);

        assert!(
            matches!(app.take_action(), Action::LaunchSsh(args) if args.contains(&"box".to_string())),
            "full screen should hand ssh the terminal"
        );
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn asking_puts_the_question_before_connecting() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);

        app.request_connection(20, 40);

        assert_eq!(app.launch_style(), LaunchStyle::Ask, "asking is the default");
        assert_eq!(app.mode, Mode::ChooseLaunch);
        assert!(matches!(app.take_action(), Action::Continue), "nothing has started yet");
    }

    #[test]
    fn a_chosen_way_is_kept_for_next_time() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.open_settings();
        app.settings_cursor_down();

        app.apply_settings_choice(&crate::test_support::StubThemeRepo);

        assert_eq!(app.launch_style(), LaunchStyle::Tab);
        assert_eq!(app.mode, Mode::Settings, "the panel stays open for the next change");
    }

    #[test]
    fn the_settings_panel_reaches_the_theme_and_the_transparency() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        let was_transparent = app.theme.transparent;
        app.open_settings();

        app.settings_cursor = 4;
        app.apply_settings_choice(&crate::test_support::StubThemeRepo);
        assert_eq!(app.theme.transparent, !was_transparent, "transparency should have flipped");

        app.settings_cursor = 3;
        app.apply_settings_choice(&crate::test_support::StubThemeRepo);
        assert_eq!(app.mode, Mode::SelectTheme, "the theme row opens the picker");

        app.cancel_mode();
        assert_eq!(app.mode, Mode::Settings, "and Esc comes back to the settings");
    }

    #[test]
    fn browsing_themes_wears_them_straight_away() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.open_theme_selector();

        app.theme_cursor_down();

        assert_eq!(app.theme.name, "other", "the screen should already be wearing it");
        assert_eq!(
            app.theme_preference.theme_index, 0,
            "nothing is saved until it is chosen"
        );
    }

    #[test]
    fn leaving_the_picker_puts_the_old_theme_back() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.open_theme_selector();
        app.theme_cursor_down();

        app.cancel_mode();

        assert_eq!(app.theme.name, "stub");
    }

    #[test]
    fn choosing_a_theme_keeps_it() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        app.open_theme_selector();
        app.theme_cursor_down();

        app.apply_selected_theme(&crate::test_support::StubThemeRepo);

        assert_eq!(app.theme.name, "other");
        assert_eq!(app.theme_preference.theme_index, 1);
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

    /// Runs a process in a tab of its own and waits for it to be over, which
    /// is what logging out of a session looks like from here.
    fn tab_running(app: &mut AppService, alias: &str, program: &str, args: &[String]) {
        let session = Session::spawn(alias, program, args, 20, 40).expect("the pty should start");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while session.is_running() {
            assert!(std::time::Instant::now() < deadline, "the session never finished");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        app.sessions.push(session);
        app.select_tab(app.sessions.len() - 1);
    }

    #[test]
    fn a_session_that_logged_out_closes_its_own_tab() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        tab_running(&mut app, "box", "true", &[]);

        app.reap_finished_sessions();

        assert!(app.sessions.is_empty());
        assert_eq!(app.active_tab, None);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn a_session_left_on_a_ctrl_c_closes_its_tab_too() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        tab_running(&mut app, "box", "sh", &["-c".into(), "exit 130".into()]);

        app.reap_finished_sessions();

        assert!(app.sessions.is_empty());
    }

    #[test]
    fn a_session_ssh_never_got_going_keeps_its_tab_so_it_can_be_read() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        tab_running(&mut app, "box", "sh", &["-c".into(), "exit 255".into()]);

        app.reap_finished_sessions();

        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.active_tab, Some(0));
    }

    #[test]
    fn reaping_leaves_the_tabs_that_are_still_going() {
        let (mut app, _repo) = app_with(vec![host("box", 22)]);
        tab_running(&mut app, "gone", "true", &[]);
        app.sessions.push(
            Session::spawn("busy", "sleep", &["5".to_string()], 20, 40).expect("the pty starts"),
        );

        app.reap_finished_sessions();

        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.sessions[0].alias, "busy");
        assert_eq!(app.active_tab, Some(0));
    }
}
