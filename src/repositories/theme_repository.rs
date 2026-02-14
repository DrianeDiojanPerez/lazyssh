use std::fs;
use std::path::PathBuf;

use crate::models::{Rgb, Theme, ThemePreference};

pub trait ThemeRepository {
    fn load_preference(&self) -> ThemePreference;
    fn save_preference(&self, preference: &ThemePreference);
    fn catalog(&self) -> Vec<Theme>;
}

pub struct FileThemeRepository {
    path: PathBuf,
}

impl FileThemeRepository {
    pub fn new() -> Self {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ssh-manager");
        fs::create_dir_all(&dir).ok();
        Self { path: dir.join("theme.json") }
    }
}

impl ThemeRepository for FileThemeRepository {
    fn load_preference(&self) -> ThemePreference {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_preference(&self, preference: &ThemePreference) {
        if let Ok(json) = serde_json::to_string_pretty(preference) {
            fs::write(&self.path, json).ok();
        }
    }

    fn catalog(&self) -> Vec<Theme> {
        ThemeCatalog::all()
    }
}

struct ThemeCatalog;

impl ThemeCatalog {
    fn all() -> Vec<Theme> {
        vec![
            Self::transparent(),
            Self::tokyo_night(),
            Self::catppuccin_mocha(),
            Self::dracula(),
            Self::nord(),
            Self::gruvbox_dark(),
            Self::cyberpunk(),
        ]
    }

    fn transparent() -> Theme {
        Theme {
            name: "Transparent".into(),
            transparent: true,
            bg: Rgb::new(0, 0, 0),
            fg: Rgb::new(220, 220, 220),
            accent: Rgb::new(114, 160, 250),
            accent_secondary: Rgb::new(180, 140, 250),
            border: Rgb::new(100, 100, 100),
            border_focused: Rgb::new(114, 160, 250),
            header_bg: Rgb::new(0, 0, 0),
            header_fg: Rgb::new(150, 220, 140),
            selected_bg: Rgb::new(60, 60, 80),
            selected_fg: Rgb::new(255, 200, 100),
            status_bar_bg: Rgb::new(0, 0, 0),
            status_bar_fg: Rgb::new(180, 180, 180),
            error: Rgb::new(255, 100, 100),
            success: Rgb::new(100, 230, 100),
            warning: Rgb::new(255, 200, 80),
            muted: Rgb::new(120, 120, 120),
            input_bg: Rgb::new(0, 0, 0),
            input_fg: Rgb::new(220, 220, 220),
            input_cursor: Rgb::new(114, 160, 250),
        }
    }

    fn tokyo_night() -> Theme {
        Theme {
            name: "Tokyo Night".into(),
            transparent: false,
            bg: Rgb::new(26, 27, 38),
            fg: Rgb::new(169, 177, 214),
            accent: Rgb::new(122, 162, 247),
            accent_secondary: Rgb::new(187, 154, 247),
            border: Rgb::new(59, 66, 97),
            border_focused: Rgb::new(122, 162, 247),
            header_bg: Rgb::new(36, 40, 59),
            header_fg: Rgb::new(195, 232, 141),
            selected_bg: Rgb::new(55, 62, 98),
            selected_fg: Rgb::new(224, 175, 104),
            status_bar_bg: Rgb::new(36, 40, 59),
            status_bar_fg: Rgb::new(169, 177, 214),
            error: Rgb::new(247, 118, 142),
            success: Rgb::new(158, 206, 106),
            warning: Rgb::new(224, 175, 104),
            muted: Rgb::new(86, 95, 137),
            input_bg: Rgb::new(36, 40, 59),
            input_fg: Rgb::new(195, 202, 251),
            input_cursor: Rgb::new(122, 162, 247),
        }
    }

    fn catppuccin_mocha() -> Theme {
        Theme {
            name: "Catppuccin Mocha".into(),
            transparent: false,
            bg: Rgb::new(30, 30, 46),
            fg: Rgb::new(205, 214, 244),
            accent: Rgb::new(137, 180, 250),
            accent_secondary: Rgb::new(203, 166, 247),
            border: Rgb::new(69, 71, 90),
            border_focused: Rgb::new(137, 180, 250),
            header_bg: Rgb::new(49, 50, 68),
            header_fg: Rgb::new(166, 227, 161),
            selected_bg: Rgb::new(69, 71, 90),
            selected_fg: Rgb::new(249, 226, 175),
            status_bar_bg: Rgb::new(24, 24, 37),
            status_bar_fg: Rgb::new(186, 194, 222),
            error: Rgb::new(243, 139, 168),
            success: Rgb::new(166, 227, 161),
            warning: Rgb::new(249, 226, 175),
            muted: Rgb::new(108, 112, 134),
            input_bg: Rgb::new(49, 50, 68),
            input_fg: Rgb::new(205, 214, 244),
            input_cursor: Rgb::new(137, 180, 250),
        }
    }

    fn dracula() -> Theme {
        Theme {
            name: "Dracula".into(),
            transparent: false,
            bg: Rgb::new(40, 42, 54),
            fg: Rgb::new(248, 248, 242),
            accent: Rgb::new(139, 233, 253),
            accent_secondary: Rgb::new(189, 147, 249),
            border: Rgb::new(68, 71, 90),
            border_focused: Rgb::new(139, 233, 253),
            header_bg: Rgb::new(55, 58, 77),
            header_fg: Rgb::new(80, 250, 123),
            selected_bg: Rgb::new(68, 71, 90),
            selected_fg: Rgb::new(255, 184, 108),
            status_bar_bg: Rgb::new(33, 34, 44),
            status_bar_fg: Rgb::new(248, 248, 242),
            error: Rgb::new(255, 85, 85),
            success: Rgb::new(80, 250, 123),
            warning: Rgb::new(241, 250, 140),
            muted: Rgb::new(98, 114, 164),
            input_bg: Rgb::new(55, 58, 77),
            input_fg: Rgb::new(248, 248, 242),
            input_cursor: Rgb::new(255, 121, 198),
        }
    }

    fn nord() -> Theme {
        Theme {
            name: "Nord".into(),
            transparent: false,
            bg: Rgb::new(46, 52, 64),
            fg: Rgb::new(216, 222, 233),
            accent: Rgb::new(136, 192, 208),
            accent_secondary: Rgb::new(180, 142, 173),
            border: Rgb::new(67, 76, 94),
            border_focused: Rgb::new(136, 192, 208),
            header_bg: Rgb::new(59, 66, 82),
            header_fg: Rgb::new(163, 190, 140),
            selected_bg: Rgb::new(76, 86, 106),
            selected_fg: Rgb::new(235, 203, 139),
            status_bar_bg: Rgb::new(59, 66, 82),
            status_bar_fg: Rgb::new(216, 222, 233),
            error: Rgb::new(191, 97, 106),
            success: Rgb::new(163, 190, 140),
            warning: Rgb::new(235, 203, 139),
            muted: Rgb::new(107, 112, 137),
            input_bg: Rgb::new(59, 66, 82),
            input_fg: Rgb::new(229, 233, 240),
            input_cursor: Rgb::new(136, 192, 208),
        }
    }

    fn gruvbox_dark() -> Theme {
        Theme {
            name: "Gruvbox Dark".into(),
            transparent: false,
            bg: Rgb::new(40, 40, 40),
            fg: Rgb::new(235, 219, 178),
            accent: Rgb::new(131, 165, 152),
            accent_secondary: Rgb::new(211, 134, 155),
            border: Rgb::new(80, 73, 69),
            border_focused: Rgb::new(131, 165, 152),
            header_bg: Rgb::new(60, 56, 54),
            header_fg: Rgb::new(184, 187, 38),
            selected_bg: Rgb::new(80, 73, 69),
            selected_fg: Rgb::new(250, 189, 47),
            status_bar_bg: Rgb::new(50, 48, 47),
            status_bar_fg: Rgb::new(235, 219, 178),
            error: Rgb::new(251, 73, 52),
            success: Rgb::new(184, 187, 38),
            warning: Rgb::new(250, 189, 47),
            muted: Rgb::new(146, 131, 116),
            input_bg: Rgb::new(60, 56, 54),
            input_fg: Rgb::new(235, 219, 178),
            input_cursor: Rgb::new(254, 128, 25),
        }
    }

    fn cyberpunk() -> Theme {
        Theme {
            name: "Cyberpunk".into(),
            transparent: false,
            bg: Rgb::new(13, 2, 33),
            fg: Rgb::new(220, 220, 255),
            accent: Rgb::new(0, 255, 255),
            accent_secondary: Rgb::new(255, 0, 255),
            border: Rgb::new(45, 20, 80),
            border_focused: Rgb::new(0, 255, 255),
            header_bg: Rgb::new(30, 10, 60),
            header_fg: Rgb::new(0, 255, 128),
            selected_bg: Rgb::new(50, 0, 100),
            selected_fg: Rgb::new(255, 255, 0),
            status_bar_bg: Rgb::new(20, 5, 50),
            status_bar_fg: Rgb::new(0, 255, 255),
            error: Rgb::new(255, 50, 80),
            success: Rgb::new(0, 255, 128),
            warning: Rgb::new(255, 200, 0),
            muted: Rgb::new(100, 80, 140),
            input_bg: Rgb::new(30, 10, 60),
            input_fg: Rgb::new(220, 220, 255),
            input_cursor: Rgb::new(255, 0, 255),
        }
    }
}
