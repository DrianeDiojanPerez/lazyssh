use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
};

use crate::models::FormField;
use crate::services::AppService;

fn centered_popup(width_pct: u16, height_pct: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(vertical[1])[1]
}

pub fn draw_form(frame: &mut Frame, app: &AppService, title: &str) {
    let t = &app.theme;
    let area = centered_popup(60, 55, frame.size());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(t.accent())
        .title(Span::styled(title, t.title()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let fields = FormField::all();

    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in &fields {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(2));
    constraints.push(Constraint::Min(0));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, field) in fields.iter().enumerate() {
        let label_row = i * 3;
        let value_row = i * 3 + 1;
        let is_active = *field == app.form_field;

        let label_style = if is_active { t.bold_accent() } else { t.muted() };

        let label_line = Line::from(vec![
            Span::styled(format!("  {} ", field.label()), label_style),
            Span::styled(format!("({})", field.placeholder()), t.muted()),
        ]);
        frame.render_widget(Paragraph::new(label_line), rows[label_row]);

        let value = read_field_display(&app.form_draft, field);
        let input_style = if is_active { t.input() } else { t.base() };
        let cursor = if is_active { "▎" } else { "" };

        let value_line = Line::from(vec![
            Span::styled(format!("  {}", value), input_style),
            Span::styled(cursor, Style::default().fg(t.input_cursor.to_color())),
        ]);
        frame.render_widget(Paragraph::new(value_line), rows[value_row]);
    }

    let footer_row = fields.len() * 3;
    if footer_row < rows.len() {
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Tab", t.bold_accent()),
            Span::styled(" next  ", t.muted()),
            Span::styled("S-Tab", t.bold_accent()),
            Span::styled(" prev  ", t.muted()),
            Span::styled("Ctrl+S / Enter", t.bold_accent()),
            Span::styled(" save  ", t.muted()),
            Span::styled("Esc", t.bold_accent()),
            Span::styled(" cancel", t.muted()),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(footer, rows[footer_row]);
    }
}

fn read_field_display(host: &crate::models::SshHost, field: &FormField) -> String {
    match field {
        FormField::Alias => host.alias.clone(),
        FormField::HostName => host.hostname.clone(),
        FormField::Port => host.port.to_string(),
        FormField::User => host.user.clone(),
        FormField::IdentityFile => host.identity_file.clone(),
    }
}

pub fn draw_delete_confirmation(frame: &mut Frame, app: &AppService, index: usize) {
    let t = &app.theme;
    let area = centered_popup(50, 28, frame.size());
    frame.render_widget(Clear, area);

    let alias = app
        .host_at(index)
        .map(|h| h.alias.as_str())
        .unwrap_or("?");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(t.error())
        .title(Span::styled(" Confirm Delete ", t.bold_error()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 1))
        .style(t.base());

    let body = Text::from(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Remove 'Host {}' from ~/.ssh/config?", alias),
            t.base().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("A backup will be created first.", t.muted())),
        Line::from(""),
        Line::from(vec![
            Span::styled("y", t.bold_error()),
            Span::styled(" confirm    ", t.muted()),
            Span::styled("n / Esc", t.bold_accent()),
            Span::styled(" cancel", t.muted()),
        ]),
    ]);

    frame.render_widget(
        Paragraph::new(body).block(block).alignment(Alignment::Center),
        area,
    );
}

pub fn draw_theme_selector(frame: &mut Frame, app: &AppService) {
    let t = &app.theme;
    let area = centered_popup(55, 60, frame.size());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(t.accent_secondary())
        .title(Span::styled(" Select Theme ", t.title()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    for (i, theme) in app.available_themes.iter().enumerate() {
        let is_pointed = i == app.theme_cursor;
        let is_active = i == app.theme_preference.theme_index;

        let pointer = if is_pointed { " ▸ " } else { "   " };
        let badge = if is_active { " (active)" } else { "" };

        let name_style = if is_pointed {
            Style::default().fg(theme.accent.to_color()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg.to_color())
        };

        let trans_mark = if theme.transparent { " [T]" } else { "" };

        lines.push(Line::from(vec![
            Span::styled(pointer, name_style),
            Span::styled("██", Style::default().fg(theme.accent.to_color())),
            Span::styled("██", Style::default().fg(theme.accent_secondary.to_color())),
            Span::styled("██", Style::default().fg(theme.success.to_color())),
            Span::styled("██", Style::default().fg(theme.warning.to_color())),
            Span::styled("  ", name_style),
            Span::styled(&theme.name, name_style),
            Span::styled(trans_mark, name_style),
            Span::styled(badge, Style::default().fg(theme.success.to_color())),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Stored in ~/.config/ssh-manager/theme.json",
        t.muted(),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("↑/↓", t.bold_accent()),
        Span::styled(" navigate  ", t.muted()),
        Span::styled("Enter", t.bold_accent()),
        Span::styled(" apply  ", t.muted()),
        Span::styled("Esc", t.bold_accent()),
        Span::styled(" close", t.muted()),
    ]));

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

pub fn draw_help(frame: &mut Frame, app: &AppService) {
    let t = &app.theme;
    let area = centered_popup(60, 78, frame.size());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(t.accent())
        .title(Span::styled(" Help ", t.title()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(3, 3, 1, 0))
        .style(t.base());

    let k = t.bold_accent();
    let d = t.base();
    let section = t.bold_accent_secondary();

    let lines = vec![
        Line::from(Span::styled("Reads and writes ~/.ssh/config directly.", d)),
        Line::from(Span::styled("A backup is created before every change.", t.muted())),
        Line::from(""),
        Line::from(Span::styled("Navigation", section)),
        Line::from(""),
        help_row("  ↑ / k         ", "Move up", k, d),
        help_row("  ↓ / j         ", "Move down", k, d),
        help_row("  g / G         ", "Jump to top / bottom", k, d),
        Line::from(""),
        Line::from(Span::styled("Actions", section)),
        Line::from(""),
        help_row("  Enter         ", "SSH into selected host", k, d),
        help_row("  a             ", "Add new host", k, d),
        help_row("  e             ", "Edit selected host", k, d),
        help_row("  d             ", "Delete (with backup)", k, d),
        help_row("  c             ", "Toggle SSH command display", k, d),
        help_row("  /             ", "Search hosts", k, d),
        help_row("  r             ", "Reload from disk", k, d),
        Line::from(""),
        Line::from(Span::styled("Appearance", section)),
        Line::from(""),
        help_row("  t             ", "Theme selector", k, d),
        help_row("  T             ", "Toggle transparency", k, d),
        Line::from(""),
        Line::from(Span::styled("Form", section)),
        Line::from(""),
        help_row("  Tab / S-Tab   ", "Next / previous field", k, d),
        help_row("  Ctrl+S/Enter  ", "Save to ~/.ssh/config", k, d),
        help_row("  Esc           ", "Cancel / close", k, d),
        Line::from(""),
        Line::from(Span::styled("Press Esc to close", t.muted())),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn help_row<'a>(key: &'a str, desc: &'a str, key_style: Style, desc_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(key, key_style),
        Span::styled(desc, desc_style),
    ])
}
