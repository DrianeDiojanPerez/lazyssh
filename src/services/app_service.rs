use std::path::PathBuf;
use std::time::Duration;

use crate::models::{Action, FormField, Mode, SshHost, Theme, ThemePreference, Toast};
use crate::repositories::{SshRepository, ThemeRepository};

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
    pub toasts: Vec<Toast>,
    // INFO: a rejected form stays open, so its complaint belongs inside the
    // form rather than in a toast that flies away
    pub form_error: Option<String>,
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
            form_port: String::new(),

            theme,
            theme_preference: preference,
            available_themes,
            theme_cursor: theme_index,

            search_query: String::new(),
            visible_indices: (0..host_count).collect(),
            show_command: true,
            toasts: Vec::new(),
            form_error: None,
            pending_action: Action::Continue,
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
        self.form_draft = SshHost::empty();
        self.form_port = String::new();
        self.form_field = FormField::Alias;
        self.mode = Mode::AddHost;
    }

    pub fn begin_edit(&mut self) {
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
