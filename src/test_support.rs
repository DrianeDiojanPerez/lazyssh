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

    fn identity_files(&self) -> Vec<String> {
        vec![
            "~/.ssh/id_ed25519".into(),
            "~/.ssh/id_rsa".into(),
            "~/.ssh/work_ed25519".into(),
        ]
    }
}

pub struct StubThemeRepo;

impl ThemeRepository for StubThemeRepo {
    fn load_preference(&self) -> ThemePreference {
        ThemePreference::default()
    }

    fn save_preference(&self, _preference: &ThemePreference) {}

    /// Every colour is distinct so a test can tell which style a cell was
    /// drawn in, not just which character.
    fn catalog(&self) -> Vec<Theme> {
        let c = |n: u8| Rgb::new(n, n, n);
        vec![Theme {
            name: "stub".into(),
            transparent: true,
            bg: c(1),
            fg: c(2),
            accent: c(3),
            accent_secondary: c(4),
            border: c(5),
            border_focused: c(6),
            header_bg: c(7),
            header_fg: c(8),
            selected_bg: c(9),
            selected_fg: c(10),
            status_bar_bg: c(11),
            status_bar_fg: c(12),
            error: c(13),
            success: c(14),
            warning: c(15),
            muted: c(16),
            input_bg: c(17),
            input_fg: c(18),
            input_cursor: c(19),
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
