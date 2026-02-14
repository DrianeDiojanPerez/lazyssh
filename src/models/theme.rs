use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_color(&self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub transparent: bool,
    pub bg: Rgb,
    pub fg: Rgb,
    pub accent: Rgb,
    pub accent_secondary: Rgb,
    pub border: Rgb,
    pub border_focused: Rgb,
    pub header_bg: Rgb,
    pub header_fg: Rgb,
    pub selected_bg: Rgb,
    pub selected_fg: Rgb,
    pub status_bar_bg: Rgb,
    pub status_bar_fg: Rgb,
    pub error: Rgb,
    pub success: Rgb,
    pub warning: Rgb,
    pub muted: Rgb,
    pub input_bg: Rgb,
    pub input_fg: Rgb,
    pub input_cursor: Rgb,
}

impl Theme {
    fn background(&self) -> Color {
        if self.transparent { Color::Reset } else { self.bg.to_color() }
    }

    pub fn base(&self) -> Style {
        Style::default().fg(self.fg.to_color()).bg(self.background())
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.border.to_color()).bg(self.background())
    }

    pub fn border_focused(&self) -> Style {
        Style::default().fg(self.border_focused.to_color()).bg(self.background())
    }

    pub fn header(&self) -> Style {
        Style::default()
            .fg(self.header_fg.to_color())
            .bg(if self.transparent { Color::Reset } else { self.header_bg.to_color() })
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected(&self) -> Style {
        Style::default()
            .fg(self.selected_fg.to_color())
            .bg(self.selected_bg.to_color())
            .add_modifier(Modifier::BOLD)
    }

    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent.to_color()).bg(self.background())
    }

    pub fn accent_secondary(&self) -> Style {
        Style::default().fg(self.accent_secondary.to_color()).bg(self.background())
    }

    pub fn status_bar(&self) -> Style {
        Style::default()
            .fg(self.status_bar_fg.to_color())
            .bg(if self.transparent { Color::Reset } else { self.status_bar_bg.to_color() })
    }

    pub fn error(&self) -> Style {
        Style::default().fg(self.error.to_color()).bg(self.background())
    }

    pub fn success(&self) -> Style {
        Style::default().fg(self.success.to_color()).bg(self.background())
    }

    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning.to_color()).bg(self.background())
    }

    pub fn muted(&self) -> Style {
        Style::default().fg(self.muted.to_color()).bg(self.background())
    }

    pub fn input(&self) -> Style {
        Style::default()
            .fg(self.input_fg.to_color())
            .bg(if self.transparent { Color::Reset } else { self.input_bg.to_color() })
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.accent.to_color())
            .bg(self.background())
            .add_modifier(Modifier::BOLD)
    }

    pub fn bold_accent(&self) -> Style {
        self.accent().add_modifier(Modifier::BOLD)
    }

    pub fn bold_error(&self) -> Style {
        self.error().add_modifier(Modifier::BOLD)
    }

    pub fn bold_warning(&self) -> Style {
        self.warning().add_modifier(Modifier::BOLD)
    }

    pub fn bold_accent_secondary(&self) -> Style {
        self.accent_secondary().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePreference {
    pub theme_index: usize,
    pub transparent: bool,
}

impl Default for ThemePreference {
    fn default() -> Self {
        Self {
            theme_index: 0,
            transparent: false,
        }
    }
}
