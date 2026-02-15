use ratatui::{
    layout::{Alignment, Constraint, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, Wrap,
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
        .style(t.header())
        .padding(Padding::symmetric(1, 1));


    let transparency_badge = if t.transparent { " [T]" } else { "" };

    let line = Line::from(vec![
        Span::styled("  SSH ", Style::default().fg(t.accent.to_color()).add_modifier(Modifier::BOLD)),
        Span::styled("Manager ", Style::default().fg(t.accent_secondary.to_color()).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {} hosts", app.host_count()), t.muted()),
        Span::styled(format!("  {}", app.config_path_display()), t.muted()),
        Span::styled(format!("  {}{}", t.name, transparency_badge), t.muted()),
    ]).centered();

    frame.render_widget(Paragraph::new(line).block(block), area);
}

pub fn draw_search_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border_focused())
        .title(Span::styled("  Search ", t.accent()))
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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(" ~/.ssh/config ", title_style))
        .padding(Padding::new(1, 1, 0, 0))
        .style(t.base());

    let entries = app.visible_hosts();

    if entries.is_empty() {
        let message = if app.search_query.is_empty() {
            "No hosts in ~/.ssh/config\n\nPress 'a' to add one"
        } else {
            "No hosts match your search"
        };
        frame.render_widget(
            Paragraph::new(message).style(t.muted()).alignment(Alignment::Center).block(block),
            area,
        );
        return;
    }

    let header = Row::new(["", "Alias", "HostName", "User"])
        .style(t.header())
        .height(1);

    let rows: Vec<Row> = entries
        .iter()
        .enumerate()
        .map(|(i, (_, host))| {
            let marker = if i == app.cursor { "▸" } else { " " };
            let style = if i == app.cursor { t.selected() } else { t.base() };

            Row::new([
                Cell::from(marker),
                Cell::from(host.alias.as_str()),
                Cell::from(host.display_host()),
                Cell::from(host.user.as_str()),
            ])
            .style(style)
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Percentage(30),
        Constraint::Percentage(40),
        Constraint::Percentage(28),
    ];

    frame.render_widget(Table::new(rows, widths).header(header).block(block), area);

    let max_visible = area.height.saturating_sub(4) as usize;
    if entries.len() > max_visible {
        let mut scrollbar_state = ScrollbarState::new(entries.len()).position(app.cursor);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .style(t.muted()),
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
            Paragraph::new("Select a host to view details")
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

    let ssh_command = host.as_ssh_command();

    let mut lines = vec![
        detail_row("Host          ", &host.alias, label, value),
        Line::from(""),
        detail_row("HostName      ", host.display_host(), label, value),
        Line::from(""),
        detail_row("Port          ", &port_display, label, port_style),
        Line::from(""),
        detail_row("User          ", user_display, label, user_style),
        Line::from(""),
        detail_row("IdentityFile  ", key_display, label, key_style),
    ];

    if host.has_extra_options() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("── Extra Options ──", t.accent_secondary())));
        for (k, v) in &host.extra_options {
            let formatted_key = format!("{:<14}", k);
            lines.push(Line::from(vec![
                Span::styled(formatted_key, dim),
                Span::styled(v.as_str(), value),
            ]));
        }
    }

    if app.show_command {
        lines.push(Line::from(""));
        lines.push(detail_row(
            "Command       ",
            &ssh_command,
            t.bold_warning(),
            t.success(),
        ));
    }

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn detail_row<'a>(label: &'a str, value: &'a str, label_style: Style, value_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(label, label_style),
        Span::styled(value, value_style),
    ])
}

pub fn draw_status_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    let (text, style) = match &app.notification {
        Some((msg, true)) => (format!(" {}", msg), t.error()),
        Some((msg, false)) => (format!(" {}", msg), t.success()),
        None => {
            let mode_label = match &app.mode {
                Mode::Normal => "NORMAL",
                Mode::Search => "SEARCH",
                Mode::AddHost => "ADD",
                Mode::EditHost(_) => "EDIT",
                Mode::ConfirmDelete(_) => "DELETE",
                Mode::SelectTheme => "THEME",
                Mode::Help => "HELP",
            };
            (format!(" {} ", mode_label), t.status_bar())
        }
    };

    let block = Block::default().title_top(Line::from(text).centered().style(style));

    let k = t.bold_accent();
    let d = t.muted();
    let sep = Span::styled(" │ ", t.border());

    let lines = vec![
        Line::from(vec![
            Span::styled("↑/k", k), Span::styled(" up ", d), sep.clone(),
            Span::styled("↓/j", k), Span::styled(" down ", d), sep.clone(),
            Span::styled("Enter", k), Span::styled(" connect ", d), sep.clone(),
            Span::styled("a", k), Span::styled(" add ", d), sep.clone(),
            Span::styled("e", k), Span::styled(" edit ", d), sep.clone(),
            Span::styled("d", k), Span::styled(" delete ", d), sep.clone(),
            Span::styled("c", k), Span::styled(" cmd ", d), sep.clone(),
            Span::styled("/", k), Span::styled(" search ", d), sep.clone(),
            Span::styled("t", k), Span::styled(" themes ", d), sep.clone(),
            Span::styled("T", k), Span::styled(" transparent ", d), sep.clone(),
            Span::styled("r", k), Span::styled(" reload ", d), sep.clone(),
            Span::styled("?", k), Span::styled(" help ", d), sep.clone(),
            Span::styled("q", k), Span::styled(" quit ", d),
        ]).centered(),
    ];

    frame.render_widget(
        Paragraph::new(lines).style(t.status_bar()).block(block),
        area,
    );
}
