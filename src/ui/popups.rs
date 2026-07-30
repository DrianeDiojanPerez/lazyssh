use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

use crate::models::FormField;
use crate::services::AppService;

/// Centres a popup of the given size inside the body area, shrinking it to
/// fit rather than letting it spill over the header and the status bar.
fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);

    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub fn draw_form(frame: &mut Frame, app: &AppService, title: &str, body: Rect) {
    let t = &app.theme;
    let fields = FormField::all();

    let error = app.notification.as_ref().filter(|(_, is_error)| *is_error);

    // two rows per field, a blank row between them, plus the footer block
    let content_height = (fields.len() * 3) as u16 + if error.is_some() { 2 } else { 1 };
    let area = centered(58, content_height + 3, body);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.accent())
        .title(Span::styled(title, t.title()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let width = block.inner(area).width as usize;
    let mut lines: Vec<Line> = Vec::new();

    for field in &fields {
        let is_active = *field == app.form_field;

        let marker = if is_active { "▸ " } else { "  " };
        let label_style = if is_active { t.bold_accent() } else { t.muted() };
        let required = if field.is_required() { "*" } else { "" };

        lines.push(Line::from(vec![
            Span::styled(format!("{}{}{}", marker, field.label(), required), label_style),
            Span::styled(format!("  {}", field.placeholder()), t.muted()),
        ]));

        let value = app.form_value(field);
        let cursor = if is_active { "▎" } else { "" };
        let input_style = if is_active { t.input() } else { t.muted() };

        lines.push(Line::from(Span::styled(
            format!("{:<width$}", format!("  {}{}", value, cursor), width = width),
            input_style,
        )));
        lines.push(Line::from(""));
    }

    if let Some((message, _)) = error {
        lines.push(Line::from(Span::styled(format!("  {}", message), t.bold_error())));
    }

    lines.push(Line::from(vec![
        Span::styled("  * required   ", t.muted()),
        Span::styled("Tab", t.bold_accent()),
        Span::styled(" next  ", t.muted()),
        Span::styled("Enter", t.bold_accent()),
        Span::styled(" save  ", t.muted()),
        Span::styled("Esc", t.bold_accent()),
        Span::styled(" cancel", t.muted()),
    ]));

    // INFO: on a terminal too short for the whole form, scroll just enough to
    // keep the field being edited on screen
    let inner_height = block.inner(area).height as usize;
    let active_row = fields.iter().position(|f| *f == app.form_field).unwrap_or(0) * 3 + 2;
    let scroll = active_row.saturating_sub(inner_height) as u16;

    frame.render_widget(Paragraph::new(lines).block(block).scroll((scroll, 0)), area);
}

pub fn draw_delete_confirmation(frame: &mut Frame, app: &AppService, index: usize, body: Rect) {
    let t = &app.theme;
    let area = centered(52, 8, body);
    frame.render_widget(Clear, area);

    let alias = app.host_at(index).map(|h| h.alias.as_str()).unwrap_or("?");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.error())
        .title(Span::styled(" Delete host ", t.bold_error()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let body = Text::from(vec![
        Line::from(Span::styled(
            format!("Remove '{}' from ~/.ssh/config?", alias),
            t.base().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("A timestamped backup is written first.", t.muted())),
        Line::from(""),
        Line::from(vec![
            Span::styled("y", t.bold_error()),
            Span::styled(" delete    ", t.muted()),
            Span::styled("Esc", t.bold_accent()),
            Span::styled(" keep", t.muted()),
        ]),
    ]);

    frame.render_widget(
        Paragraph::new(body).block(block).alignment(Alignment::Center),
        area,
    );
}

pub fn draw_theme_selector(frame: &mut Frame, app: &AppService, body: Rect) {
    let t = &app.theme;
    let height = app.available_themes.len() as u16 + 5;
    let area = centered(46, height, body);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.accent_secondary())
        .title(Span::styled(" Themes ", t.title()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let mut lines = Vec::new();

    for (i, theme) in app.available_themes.iter().enumerate() {
        let is_pointed = i == app.theme_cursor;
        let is_active = i == app.theme_preference.theme_index;

        let name_style = if is_pointed {
            Style::default().fg(theme.accent.to_color()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg.to_color())
        };

        lines.push(Line::from(vec![
            Span::styled(if is_pointed { "▸ " } else { "  " }, name_style),
            Span::styled("██", Style::default().fg(theme.accent.to_color())),
            Span::styled("██", Style::default().fg(theme.accent_secondary.to_color())),
            Span::styled("██", Style::default().fg(theme.success.to_color())),
            Span::styled("██", Style::default().fg(theme.warning.to_color())),
            Span::styled(format!("  {}", theme.name), name_style),
            Span::styled(if theme.transparent { " [T]" } else { "" }, name_style),
            Span::styled(
                if is_active { "  (active)" } else { "" },
                Style::default().fg(theme.success.to_color()),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ↑↓", t.bold_accent()),
        Span::styled(" browse  ", t.muted()),
        Span::styled("Enter", t.bold_accent()),
        Span::styled(" apply  ", t.muted()),
        Span::styled("Esc", t.bold_accent()),
        Span::styled(" close", t.muted()),
    ]));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn draw_help(frame: &mut Frame, app: &AppService, body: Rect) {
    let t = &app.theme;

    let left = [
        ("Navigation", ""),
        ("↑ / k", "move up"),
        ("↓ / j", "move down"),
        ("g / G", "top / bottom"),
        ("/", "search hosts"),
        ("Esc", "clear the filter"),
        ("", ""),
        ("Hosts", ""),
        ("Enter", "connect"),
        ("a", "add a host"),
        ("e", "edit selected"),
        ("d", "delete selected"),
        ("r", "reload from disk"),
    ];

    let right = [
        ("Form", ""),
        ("Tab", "next field"),
        ("S-Tab", "previous field"),
        ("Enter", "save"),
        ("Esc", "cancel"),
        ("", ""),
        ("Look", ""),
        ("t", "pick a theme"),
        ("T", "transparency"),
        ("", ""),
        ("Other", ""),
        ("c", "show ssh command"),
        ("q", "quit"),
    ];

    let rows = left.len().max(right.len());
    let area = centered(70, rows as u16 + 7, body);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.accent())
        .title(Span::styled(" Help ", t.title()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let mut lines = vec![
        Line::from(Span::styled(
            "Reads and writes ~/.ssh/config, backing it up on every change.",
            t.muted(),
        )),
        Line::from(""),
    ];

    for i in 0..rows {
        let mut spans = help_cell(left.get(i), t.bold_accent(), t.base(), t.bold_accent_secondary());
        spans.extend(help_cell(right.get(i), t.bold_accent(), t.base(), t.bold_accent_secondary()));
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Esc closes this help", t.muted())));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn help_cell<'a>(
    entry: Option<&(&'a str, &'a str)>,
    key_style: Style,
    desc_style: Style,
    section_style: Style,
) -> Vec<Span<'a>> {
    let Some((key, desc)) = entry else {
        return vec![Span::raw(format!("{:<32}", ""))];
    };

    if desc.is_empty() {
        return vec![Span::styled(format!("{:<32}", key), section_style)];
    }

    vec![
        Span::styled(format!("  {:<8}", key), key_style),
        Span::styled(format!("{:<22}", desc), desc_style),
    ]
}
