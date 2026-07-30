use ratatui::{
    layout::{Alignment, Constraint, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
    Frame,
};

use crate::models::Mode;
use crate::services::AppService;

pub fn draw_header(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Thick)
        .border_style(t.accent())
        .style(t.header());

    let transparency_badge = if t.transparent { " [T]" } else { "" };

    let line = Line::from(vec![
        Span::styled(" lazyssh ", Style::default().fg(t.accent.to_color()).add_modifier(Modifier::BOLD)),
        Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), t.muted()),
        Span::styled("   ", t.muted()),
        Span::styled(app.config_path_display(), t.accent_secondary()),
        Span::styled(format!("   {} hosts", app.host_count()), t.muted()),
        Span::styled(format!("   {}{}", t.name, transparency_badge), t.muted()),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

pub fn draw_search_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border_focused())
        .title(Span::styled(" Search ", t.accent()))
        .style(t.input());

    let line = Line::from(vec![
        Span::styled(" ", t.input()),
        Span::styled(&app.search_query, t.input()),
        Span::styled("▎", Style::default().fg(t.input_cursor.to_color())),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

pub fn draw_host_list(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;
    let is_focused = matches!(app.mode, Mode::Normal | Mode::Search);

    let border = if is_focused { t.border_focused() } else { t.border() };
    let title_style = if is_focused { t.title() } else { t.muted() };

    let entries = app.visible_hosts();

    let title = if app.has_filter() {
        format!(" Hosts {}/{}  '{}' ", entries.len(), app.host_count(), ellipsize(&app.search_query, 12))
    } else {
        format!(" Hosts {} ", app.host_count())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(title, title_style))
        .padding(Padding::new(1, 1, 0, 0))
        .style(t.base());

    if entries.is_empty() {
        let message = if app.has_filter() {
            "No hosts match this search\n\nPress Esc to clear the filter"
        } else {
            "No hosts in ~/.ssh/config\n\nPress 'a' to add one"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(t.muted())
                .alignment(Alignment::Center)
                .block(block),
            area,
        );
        return;
    }

    let header = Row::new(["Alias", "HostName", "User"])
        .style(t.header())
        .height(1);

    let rows: Vec<Row> = entries
        .iter()
        .map(|(_, host)| {
            let target = if host.has_custom_port() {
                format!("{}:{}", host.display_host(), host.port)
            } else {
                host.display_host().to_string()
            };

            let user = if host.user.is_empty() { "-" } else { host.user.as_str() };

            Row::new([
                Cell::from(host.alias.as_str()),
                Cell::from(target),
                Cell::from(user),
            ])
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(36),
        Constraint::Percentage(42),
        Constraint::Percentage(22),
    ];

    // INFO: TableState owns the scroll offset, which is what keeps the
    // selected host on screen once the list is taller than the panel
    let mut state = TableState::default().with_selected(Some(app.cursor));

    frame.render_stateful_widget(
        Table::new(rows, widths)
            .header(header)
            .block(block)
            .highlight_symbol("▸ ")
            .highlight_style(t.selected()),
        area,
        &mut state,
    );

    let viewport = area.height.saturating_sub(3) as usize;
    if entries.len() > viewport {
        let mut scrollbar_state = ScrollbarState::new(entries.len()).position(app.cursor);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(t.border()),
            area.inner(&Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

pub fn draw_detail_panel(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border())
        .title(Span::styled(" Details ", t.title()))
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let Some(host) = app.selected_host() else {
        frame.render_widget(
            Paragraph::new("No host selected")
                .style(t.muted())
                .alignment(Alignment::Center)
                .block(block),
            area,
        );
        return;
    };

    let label = t.bold_accent();
    let value = t.base();
    let dim = t.muted();

    let port_display = host.port.to_string();
    let port_style = if host.has_custom_port() { value } else { dim };

    let user_display: &str = if host.user.is_empty() { "(default)" } else { &host.user };
    let user_style = if host.user.is_empty() { dim } else { value };

    let key_display: &str = if host.has_identity_file() { &host.identity_file } else { "(default)" };
    let key_style = if host.has_identity_file() { value } else { dim };

    let mut lines = vec![Line::from(Span::styled(host.alias.as_str(), t.title()))];

    if app.show_command {
        lines.push(Line::from(Span::styled(host.as_ssh_command(), t.success())));
    }

    lines.extend([
        Line::from(""),
        detail_row("HostName ", host.display_host(), label, value),
        detail_row("Port     ", &port_display, label, port_style),
        detail_row("User     ", user_display, label, user_style),
        detail_row("Identity ", key_display, label, key_style),
    ]);

    if host.has_extra_options() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Extra options", t.accent_secondary())));
        for (k, v) in &host.extra_options {
            lines.push(Line::from(vec![
                Span::styled(format!("{:<10}", k), dim),
                Span::styled(v.as_str(), value),
            ]));
        }
    }


    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn ellipsize(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit.saturating_sub(1)).chain("…".chars()).collect()
}

fn detail_row<'a>(label: &'a str, value: &'a str, label_style: Style, value_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(label, label_style),
        Span::styled(value, value_style),
    ])
}

pub fn draw_status_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    if let Some((message, is_error)) = &app.notification {
        let (icon, style) = if *is_error {
            ("  ✖ ", t.bold_error())
        } else {
            ("  ✔ ", t.success())
        };

        let line = Line::from(vec![
            mode_chip(app),
            Span::styled(format!("{}{}", icon, message), style),
        ]);

        frame.render_widget(Paragraph::new(line).style(t.status_bar()), area);
        return;
    }

    let mut spans = vec![mode_chip(app), Span::styled("  ", t.status_bar())];
    let mut used: usize = spans.iter().map(|s| s.content.chars().count()).sum();

    let separator = Span::styled(" · ", t.border());
    // INFO: the way out of any mode is worth more than the rest of the hints,
    // so its width is reserved before the others are laid out
    let escape = escape_hint(&app.mode);
    let reserved = escape.0.chars().count() + escape.1.chars().count() + 4;

    for (key, label) in hints_for(&app.mode) {
        let width = key.chars().count() + label.chars().count() + 4;
        if used + width + reserved > area.width as usize {
            break;
        }
        used += width;

        spans.push(Span::styled(*key, t.bold_accent()));
        spans.push(Span::styled(format!(" {}", label), t.muted()));
        spans.push(separator.clone());
    }

    spans.push(Span::styled(escape.0, t.bold_accent()));
    spans.push(Span::styled(format!(" {}", escape.1), t.muted()));

    frame.render_widget(Paragraph::new(Line::from(spans)).style(t.status_bar()), area);
}

fn mode_chip<'a>(app: &AppService) -> Span<'a> {
    let label = match &app.mode {
        Mode::Normal => "NORMAL",
        Mode::Search => "SEARCH",
        Mode::AddHost => "ADD",
        Mode::EditHost(_) => "EDIT",
        Mode::ConfirmDelete(_) => "DELETE",
        Mode::SelectTheme => "THEME",
        Mode::Help => "HELP",
    };

    Span::styled(format!(" {} ", label), app.theme.selected())
}

/// The one hint that always stays visible: how to leave the current mode.
fn escape_hint(mode: &Mode) -> (&'static str, &'static str) {
    match mode {
        Mode::Normal => ("?", "help"),
        Mode::Search => ("Esc", "clear"),
        Mode::AddHost | Mode::EditHost(_) => ("Esc", "cancel"),
        Mode::ConfirmDelete(_) => ("Esc", "keep"),
        Mode::SelectTheme | Mode::Help => ("Esc", "close"),
    }
}

/// The hints that matter in the current mode, most useful first, so the
/// status bar degrades sensibly on a narrow terminal.
fn hints_for(mode: &Mode) -> &'static [(&'static str, &'static str)] {
    match mode {
        Mode::Normal => &[
            ("↵", "connect"),
            ("a", "add"),
            ("e", "edit"),
            ("d", "delete"),
            ("/", "search"),
            ("q", "quit"),
            ("↑↓", "move"),
            ("c", "command"),
            ("r", "reload"),
            ("t", "theme"),
            ("T", "transparency"),
        ],
        Mode::Search => &[
            ("type", "to filter"),
            ("↵", "keep filter"),
            ("↑↓", "move"),
        ],
        Mode::AddHost | Mode::EditHost(_) => &[
            ("Tab", "next field"),
            ("S-Tab", "previous"),
            ("↵", "save"),
        ],
        Mode::ConfirmDelete(_) => &[("y", "delete")],
        Mode::SelectTheme => &[("↑↓", "browse"), ("↵", "apply")],
        Mode::Help => &[],
    }
}
