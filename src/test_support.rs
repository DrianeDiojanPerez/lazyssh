use std::cell::RefCell;
use std::path::PathBuf;

use crate::models::{Rgb, SshHost, Theme, ThemePreference};
use crate::repositories::{SshRepository, ThemeRepository};
use crate::services::AppService;

pub struct StubSshRepo {
    stored: RefCell<Vec<SshHost>>,
}

impl StubSshRepo {
    pub fn with(hosts: Vec<SshHost>) -> Self {
        Self { stored: RefCell::new(hosts) }
    }
}

impl SshRepository for StubSshRepo {
    fn load_all(&self) -> (String, Vec<SshHost>) {
        (String::new(), self.stored.borrow().clone())
    }

    fn save_all(&self, _preamble: &str, hosts: &[SshHost]) -> Result<PathBuf, String> {
        *self.stored.borrow_mut() = hosts.to_vec();
        Ok(PathBuf::from("/stub/config"))
    }

    fn config_path(&self) -> PathBuf {
        PathBuf::from("/home/tester/.ssh/config")
    }
}

pub struct StubThemeRepo;

impl ThemeRepository for StubThemeRepo {
    fn load_preference(&self) -> ThemePreference {
        ThemePreference::default()
    }

    fn save_preference(&self, _preference: &ThemePreference) {}

    fn catalog(&self) -> Vec<Theme> {
        let c = || Rgb::new(0, 0, 0);
        vec![Theme {
            name: "stub".into(),
            transparent: true,
            bg: c(),
            fg: c(),
            accent: c(),
            accent_secondary: c(),
            border: c(),
            border_focused: c(),
            header_bg: c(),
            header_fg: c(),
            selected_bg: c(),
            selected_fg: c(),
            status_bar_bg: c(),
            status_bar_fg: c(),
            error: c(),
            success: c(),
            warning: c(),
            muted: c(),
            input_bg: c(),
            input_fg: c(),
            input_cursor: c(),
        }]
    }
}

pub fn host(alias: &str, port: u16) -> SshHost {
    SshHost {
        alias: alias.into(),
        hostname: format!("{}.example.com", alias),
        port,
        user: "dperez".into(),
        ..SshHost::empty()
    }
}

pub fn app_with(hosts: Vec<SshHost>) -> (AppService, StubSshRepo) {
    let ssh_repo = StubSshRepo::with(hosts);
    let app = AppService::initialize(&ssh_repo, &StubThemeRepo);
    (app, ssh_repo)
}
