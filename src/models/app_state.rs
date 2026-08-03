use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Search,
    AddHost,
    EditHost(usize),
    ConfirmDelete(usize),
    SelectTheme,
    ChooseLaunch,
    Settings,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LaunchStyle {
    Ask,
    Tab,
    FullScreen,
}

impl LaunchStyle {
    pub fn all() -> [Self; 3] {
        [Self::Ask, Self::Tab, Self::FullScreen]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Ask => "Ask every time",
            Self::Tab => "Open in a tab",
            Self::FullScreen => "Take the whole terminal",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormField {
    Alias,
    HostName,
    Port,
    User,
    IdentityFile,
}

impl FormField {
    pub fn all() -> Vec<Self> {
        vec![Self::Alias, Self::HostName, Self::Port, Self::User, Self::IdentityFile]
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Alias => Self::HostName,
            Self::HostName => Self::Port,
            Self::Port => Self::User,
            Self::User => Self::IdentityFile,
            Self::IdentityFile => Self::Alias,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Alias => Self::IdentityFile,
            Self::HostName => Self::Alias,
            Self::Port => Self::HostName,
            Self::User => Self::Port,
            Self::IdentityFile => Self::User,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Alias => "Host Alias",
            Self::HostName => "HostName",
            Self::Port => "Port",
            Self::User => "User",
            Self::IdentityFile => "IdentityFile",
        }
    }

    pub fn placeholder(&self) -> &str {
        match self {
            Self::Alias => "name used with ssh <alias>",
            Self::HostName => "IP or domain",
            Self::Port => "default 22",
            Self::User => "login username",
            Self::IdentityFile => "path to key (optional)",
        }
    }

    pub fn is_required(&self) -> bool {
        matches!(self, Self::Alias | Self::HostName)
    }

    pub fn accepts_char(&self, c: char) -> bool {
        match self {
            Self::Port => c.is_ascii_digit(),
            _ => true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Continue,
    Quit,
    LaunchSsh(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reachability {
    Unknown,
    Checking,
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Setting {
    Launch(LaunchStyle),
    Theme,
    Transparency,
    TabEdges,
    TabPanel,
}

impl Setting {
    pub fn all() -> Vec<Self> {
        let mut rows: Vec<Self> = LaunchStyle::all().iter().map(|s| Self::Launch(*s)).collect();
        rows.push(Self::Theme);
        rows.push(Self::Transparency);
        rows.push(Self::TabEdges);
        rows.push(Self::TabPanel);
        rows
    }

    /// Which line of the panel this row is drawn on, counting the headings
    /// and the blank line between the two groups.
    pub fn line(index: usize) -> u16 {
        match index {
            0..=2 => index as u16 + 1,
            other => other as u16 + 3,
        }
    }

    pub fn at_line(line: u16) -> Option<usize> {
        match line {
            1..=3 => Some(line as usize - 1),
            6..=9 => Some(line as usize - 3),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Launch(style) => style.label(),
            Self::Theme => "Theme",
            Self::Transparency => "Transparency",
            Self::TabEdges => "Slanted tabs",
            Self::TabPanel => "Tabs in a panel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Sidebar,
    Session,
}
