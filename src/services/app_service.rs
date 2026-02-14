use std::path::PathBuf;

use crate::models::{Action, FormField, Mode, SshHost, Theme, ThemePreference};
use crate::repositories::{SshRepository, ThemeRepository};

pub struct AppService {
    preamble: String,
    hosts: Vec<SshHost>,
    ssh_config_path: PathBuf,

    pub mode: Mode,
    pub cursor: usize,
    pub form_draft: SshHost,
    pub form_field: FormField,

    pub theme: Theme,
    pub theme_preference: ThemePreference,
    pub available_themes: Vec<Theme>,
    pub theme_cursor: usize,

    pub search_query: String,
    pub visible_indices: Vec<usize>,
    pub show_command: bool,
    pub notification: Option<(String, bool)>,
    pub pending_action: Action,
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

        Self {
            preamble,
            hosts,
            ssh_config_path,

            mode: Mode::Normal,
            cursor: 0,
            form_draft: SshHost::empty(),
            form_field: FormField::Alias,

            theme,
            theme_preference: preference,
            available_themes,
            theme_cursor: theme_index,

            search_query: String::new(),
            visible_indices: (0..host_count).collect(),
            show_command: false,
            notification: None,
            pending_action: Action::Continue,
        }
    }

    pub fn config_path_display(&self) -> String {
        self.ssh_config_path.to_string_lossy().to_string()
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
        self.search_query.clear();
        self.rebuild_filter();
        self.mode = Mode::Normal;
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
        self.form_draft = SshHost::empty();
        self.form_field = FormField::Alias;
        self.mode = Mode::AddHost;
    }

    pub fn begin_edit(&mut self) {
        if let Some(index) = self.selected_real_index() {
            self.form_draft = self.hosts[index].clone();
            self.form_field = FormField::Alias;
            self.mode = Mode::EditHost(index);
        }
    }

    pub fn begin_delete(&mut self) {
        if let Some(index) = self.selected_real_index() {
            self.mode = Mode::ConfirmDelete(index);
        }
    }

    pub fn commit_add(&mut self, ssh_repo: &dyn SshRepository) {
        if !self.form_draft.is_valid() {
            self.notification = Some(("Alias and HostName are required".into(), true));
            return;
        }

        let alias_exists = self.hosts.iter().any(|h| {
            h.alias.to_lowercase() == self.form_draft.alias.to_lowercase()
        });
        if alias_exists {
            self.notification = Some((
                format!("'{}' already exists", self.form_draft.alias),
                true,
            ));
            return;
        }

        let name = self.form_draft.alias.clone();
        self.hosts.push(self.form_draft.clone());

        match ssh_repo.save_all(&self.preamble, &self.hosts) {
            Ok(_) => {
                self.notification = Some((format!("Added '{}'", name), false));
            }
            Err(e) => {
                self.hosts.pop();
                self.notification = Some((e, true));
            }
        }

        self.rebuild_filter();
        self.mode = Mode::Normal;
    }

    pub fn commit_edit(&mut self, index: usize, ssh_repo: &dyn SshRepository) {
        if !self.form_draft.is_valid() {
            self.notification = Some(("Alias and HostName are required".into(), true));
            return;
        }

        let duplicate = self.hosts.iter().enumerate().any(|(i, h)| {
            i != index && h.alias.to_lowercase() == self.form_draft.alias.to_lowercase()
        });
        if duplicate {
            self.notification = Some((
                format!("'{}' already exists", self.form_draft.alias),
                true,
            ));
            return;
        }

        let name = self.form_draft.alias.clone();
        let backup = self.hosts[index].clone();
        self.hosts[index] = self.form_draft.clone();

        match ssh_repo.save_all(&self.preamble, &self.hosts) {
            Ok(_) => {
                self.notification = Some((format!("Updated '{}'", name), false));
            }
            Err(e) => {
                self.hosts[index] = backup;
                self.notification = Some((e, true));
            }
        }

        self.rebuild_filter();
        self.mode = Mode::Normal;
    }

    pub fn commit_delete(&mut self, index: usize, ssh_repo: &dyn SshRepository) {
        let removed = self.hosts.remove(index);

        match ssh_repo.save_all(&self.preamble, &self.hosts) {
            Ok(_) => {
                self.notification = Some((
                    format!("Deleted '{}'", removed.alias),
                    false,
                ));
            }
            Err(e) => {
                self.hosts.insert(index, removed);
                self.notification = Some((e, true));
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

    pub fn launch_ssh(&mut self) {
        if let Some(host) = self.selected_host() {
            self.pending_action = Action::LaunchSsh(host.as_ssh_args());
        }
    }

    // ─── Form Editing ────────────────────────────────────────────────────

    pub fn form_next_field(&mut self) {
        self.form_field = self.form_field.next();
    }

    pub fn form_previous_field(&mut self) {
        self.form_field = self.form_field.previous();
    }

    pub fn form_type_char(&mut self, c: char) {
        if !self.form_field.accepts_char(c) {
            return;
        }
        let mut value = self.read_form_field();
        value.push(c);
        self.write_form_field(value);
    }

    pub fn form_delete_char(&mut self) {
        let mut value = self.read_form_field();
        value.pop();
        self.write_form_field(value);
    }

    fn read_form_field(&self) -> String {
        match self.form_field {
            FormField::Alias => self.form_draft.alias.clone(),
            FormField::HostName => self.form_draft.hostname.clone(),
            FormField::Port => self.form_draft.port.to_string(),
            FormField::User => self.form_draft.user.clone(),
            FormField::IdentityFile => self.form_draft.identity_file.clone(),
        }
    }

    fn write_form_field(&mut self, value: String) {
        match self.form_field {
            FormField::Alias => self.form_draft.alias = value,
            FormField::HostName => self.form_draft.hostname = value,
            FormField::Port => self.form_draft.port = value.parse().unwrap_or(22),
            FormField::User => self.form_draft.user = value,
            FormField::IdentityFile => self.form_draft.identity_file = value,
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

            self.notification = Some((format!("Theme: {}", new_theme.name), false));
        }
        self.mode = Mode::Normal;
    }

    pub fn toggle_transparency(&mut self, theme_repo: &dyn ThemeRepository) {
        self.theme.transparent = !self.theme.transparent;
        self.theme_preference.transparent = self.theme.transparent;
        theme_repo.save_preference(&self.theme_preference);

        let label = if self.theme.transparent { "ON" } else { "OFF" };
        self.notification = Some((format!("Transparency: {}", label), false));
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
        self.notification = Some((
            format!("Reloaded ({} hosts)", self.hosts.len()),
            false,
        ));
    }

    pub fn open_help(&mut self) {
        self.mode = Mode::Help;
    }

    pub fn request_quit(&mut self) {
        self.pending_action = Action::Quit;
    }

    pub fn clear_notification(&mut self) {
        self.notification = None;
    }

    pub fn take_action(&mut self) -> Action {
        let action = self.pending_action.clone();
        self.pending_action = Action::Continue;
        action
    }
}
