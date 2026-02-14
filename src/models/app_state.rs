#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Search,
    AddHost,
    EditHost(usize),
    ConfirmDelete(usize),
    SelectTheme,
    Help,
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
