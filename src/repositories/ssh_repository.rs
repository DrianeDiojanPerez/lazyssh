use std::fs;
use std::path::PathBuf;

use chrono::Local;

use crate::models::SshHost;

pub trait SshRepository {
    fn load_all(&self) -> (String, Vec<SshHost>);
    fn save_all(&self, preamble: &str, hosts: &[SshHost]) -> Result<PathBuf, String>;
    fn config_path(&self) -> PathBuf;
}

pub struct FileSshRepository {
    path: PathBuf,
}

impl FileSshRepository {
    pub fn new() -> Self {
        let path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".ssh")
            .join("config");
        Self { path }
    }

    fn read_file(&self) -> String {
        fs::read_to_string(&self.path).unwrap_or_default()
    }

    fn create_backup(&self) -> Result<PathBuf, String> {
        if !self.path.exists() {
            return Ok(self.path.clone());
        }

        let backup_name = format!(
            "config.backup_{}",
            Local::now().format("%Y%m%d_%H%M%S")
        );
        let backup_path = self.path.parent().unwrap().join(backup_name);

        fs::copy(&self.path, &backup_path)
            .map_err(|e| format!("backup failed: {}", e))?;

        Ok(backup_path)
    }

    fn serialize(preamble: &str, hosts: &[SshHost]) -> String {
        let mut output = String::new();

        if !preamble.is_empty() {
            output.push_str(preamble);
            if !preamble.ends_with('\n') {
                output.push('\n');
            }
        }

        for host in hosts {
            output.push_str(&format!("Host {}\n", host.alias));

            if !host.hostname.is_empty() {
                output.push_str(&format!("    HostName {}\n", host.hostname));
            }
            if !host.user.is_empty() {
                output.push_str(&format!("    User {}\n", host.user));
            }
            if host.has_custom_port() {
                output.push_str(&format!("    Port {}\n", host.port));
            }
            if host.has_identity_file() {
                output.push_str(&format!("    IdentityFile {}\n", host.identity_file));
            }
            for (key, value) in &host.extra_options {
                output.push_str(&format!("    {} {}\n", key, value));
            }
            output.push('\n');
        }

        output
    }

    fn set_permissions(&self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600)).ok();
        }
    }
}

impl SshRepository for FileSshRepository {
    fn load_all(&self) -> (String, Vec<SshHost>) {
        let content = self.read_file();
        SshConfigParser::parse(&content)
    }

    fn save_all(&self, preamble: &str, hosts: &[SshHost]) -> Result<PathBuf, String> {
        let backup_path = self.create_backup()?;

        let ssh_dir = self.path.parent().unwrap();
        fs::create_dir_all(ssh_dir)
            .map_err(|e| format!("cannot create .ssh directory: {}", e))?;

        let content = Self::serialize(preamble, hosts);
        fs::write(&self.path, content)
            .map_err(|e| format!("write failed: {}", e))?;

        self.set_permissions();
        Ok(backup_path)
    }

    fn config_path(&self) -> PathBuf {
        self.path.clone()
    }
}

struct SshConfigParser;

impl SshConfigParser {
    fn parse(content: &str) -> (String, Vec<SshHost>) {
        let mut preamble = String::new();
        let mut hosts: Vec<SshHost> = Vec::new();
        let mut current: Option<SshHost> = None;
        let mut in_preamble = true;

        for line in content.lines() {
            let trimmed = line.trim();
            let lower = trimmed.to_lowercase();

            if lower.starts_with("host ") && !lower.starts_with("hostname") {
                if let Some(host) = current.take() {
                    Self::push_if_named(&mut hosts, host);
                }

                in_preamble = false;
                let alias = Self::strip_inline_comment(&trimmed[5..]).trim().to_string();

                current = Some(SshHost {
                    alias,
                    ..SshHost::empty()
                });
                continue;
            }

            if in_preamble {
                preamble.push_str(line);
                preamble.push('\n');
                continue;
            }

            if let Some(ref mut host) = current {
                Self::parse_option(host, trimmed);
            }
        }

        if let Some(host) = current {
            Self::push_if_named(&mut hosts, host);
        }

        (preamble, hosts)
    }

    fn push_if_named(hosts: &mut Vec<SshHost>, host: SshHost) {
        if host.alias != "*" && !host.alias.is_empty() {
            hosts.push(host);
        }
    }

    fn parse_option(host: &mut SshHost, line: &str) {
        if line.is_empty() || line.starts_with('#') {
            return;
        }

        let (key, raw_value) = Self::split_key_value(line);
        if key.is_empty() {
            return;
        }

        let value = Self::strip_inline_comment(&raw_value).to_string();

        match key.to_lowercase().as_str() {
            "hostname" => host.hostname = value,
            "port" => host.port = value.parse().unwrap_or(22),
            "user" => host.user = value,
            "identityfile" => host.identity_file = value,
            _ => host.extra_options.push((key, value)),
        }
    }

    fn split_key_value(line: &str) -> (String, String) {
        if let Some(pos) = line.find('=') {
            let k = line[..pos].trim().to_string();
            let v = line[pos + 1..].trim().to_string();
            return (k, v);
        }

        if let Some(pos) = line.find(char::is_whitespace) {
            let k = line[..pos].trim().to_string();
            let v = line[pos..].trim().to_string();
            return (k, v);
        }

        (String::new(), String::new())
    }

    fn strip_inline_comment(s: &str) -> &str {
        let mut in_quote = false;
        let mut quote_char = ' ';

        for (i, c) in s.char_indices() {
            if in_quote {
                if c == quote_char {
                    in_quote = false;
                }
            } else if c == '"' || c == '\'' {
                in_quote = true;
                quote_char = c;
            } else if c == '#' {
                return s[..i].trim_end();
            }
        }

        s
    }
}
