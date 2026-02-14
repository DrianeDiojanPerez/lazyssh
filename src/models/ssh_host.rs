use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHost {
    pub alias: String,
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub identity_file: String,
    pub extra_options: Vec<(String, String)>,
}

impl SshHost {
    pub fn empty() -> Self {
        Self {
            alias: String::new(),
            hostname: String::new(),
            port: 22,
            user: String::new(),
            identity_file: String::new(),
            extra_options: Vec::new(),
        }
    }

    pub fn display_host(&self) -> &str {
        if self.hostname.is_empty() {
            &self.alias
        } else {
            &self.hostname
        }
    }

    pub fn as_ssh_command(&self) -> String {
        format!("ssh {}", self.alias)
    }

    pub fn as_ssh_args(&self) -> Vec<String> {
        vec![self.alias.clone()]
    }

    pub fn is_valid(&self) -> bool {
        !self.alias.is_empty() && !self.hostname.is_empty()
    }

    pub fn has_custom_port(&self) -> bool {
        self.port != 22
    }

    pub fn has_identity_file(&self) -> bool {
        !self.identity_file.is_empty()
    }

    pub fn has_extra_options(&self) -> bool {
        !self.extra_options.is_empty()
    }
}
