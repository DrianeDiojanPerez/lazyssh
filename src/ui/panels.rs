use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
    Frame,
};

use crate::models::{Mode, SshHost};
use crate::services::AppService;

pub fn draw_header(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(t.border())
        .style(t.base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let brand = Line::from(vec![
        Span::styled(" lazyssh ", t.pill(&t.accent)),
        Span::styled(format!(" v{}", env!("CARGO_PKG_VERSION")), t.muted()),
    ]);

    let transparency_badge = if t.transparent { " [T]" } else { "" };
    let meta = [
        (app.config_path_display(), t.accent_secondary()),
        (format!("{} hosts", app.host_count()), t.muted()),
        (format!("{}{}", t.name, transparency_badge), t.muted()),
    ];

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(brand.width() as u16), Constraint::Min(0)])
        .split(inner);

    frame.render_widget(Paragraph::new(brand), columns[0]);
    frame.render_widget(
        Paragraph::new(join_while_it_fits(&meta, t.border(), columns[1].width).alignment(Alignment::Right)),
        columns[1],
    );
}

/// Joins segments with a dot until the next one would not fit, so the widest
/// terminal shows everything and a narrow one keeps what comes first.
fn join_while_it_fits<'a>(
    segments: &[(String, Style)],
    separator_style: Style,
    width: u16,
) -> Line<'a> {
    let mut spans: Vec<Span> = Vec::new();
    let mut used = 1;

    for (text, style) in segments {
        let separator = if spans.is_empty() { 0 } else { 3 };
        if used + separator + text.chars().count() > width as usize {
            break;
        }
        used += separator + text.chars().count();

        if separator > 0 {
            spans.push(Span::styled(" · ", separator_style));
        }
        spans.push(Span::styled(text.clone(), *style));
    }

    spans.push(Span::styled(" ", separator_style));
    Line::from(spans)
}

pub fn draw_search_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border_focused())
        .title(Span::styled(" Filter ", t.title()))
        .padding(Padding::new(1, 1, 0, 0))
        .style(t.base());

    let line = if app.search_query.is_empty() {
        Line::from(vec![
            Span::styled("/ ", t.accent()),
            Span::styled("▎", Style::default().fg(t.input_cursor.to_color())),
            Span::styled(" type to narrow the list", t.muted()),
        ])
    } else {
        Line::from(vec![
            Span::styled("/ ", t.accent()),
            Span::styled(&app.search_query, t.base().add_modifier(Modifier::BOLD)),
            Span::styled("▎", Style::default().fg(t.input_cursor.to_color())),
        ])
    };

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
        .padding(Padding::new(1, 1, 1, 0))
        .style(t.base());

    let inner = block.inner(area);

    if entries.is_empty() {
        let (headline, hint) = if app.has_filter() {
            ("Nothing matches that filter", "Esc clears it")
        } else {
            ("No hosts yet", "Press 'a' to add your first one")
        };
        draw_placeholder(frame, app, block, area, headline, hint);
        return;
    }

    // INFO: a card breathes better with a blank row after it, but on a short
    // panel that row costs a whole host, so it is the first thing to go
    let spacing = if inner.height >= 11 { 1 } else { 0 };
    // INFO: the right hand column is only worth its width when some host
    // actually has a custom port or a key to show there
    // the column carries its own two space gutter, so nothing collides
    let meta = if inner.width >= 34 { meta_width(&entries) } else { 0 };
    let meta_column = if meta > 0 { meta + 2 } else { 0 };
    let text_width = inner.width.saturating_sub(meta_column + 2) as usize;

    let rows: Vec<Row> = entries
        .iter()
        .enumerate()
        .map(|(row, (_, host))| card(app, row == app.cursor, host, text_width))
        .map(|cells| Row::new(cells).height(2).bottom_margin(spacing))
        .collect();

    let widths = [Constraint::Min(10), Constraint::Length(meta_column)];

    // INFO: TableState owns the scroll offset, which is what keeps the
    // selected host on screen once the list is taller than the panel
    let mut state = TableState::default().with_selected(Some(app.cursor));

    frame.render_stateful_widget(
        Table::new(rows, widths).block(block).column_spacing(0).highlight_symbol(Text::from(vec![
            Line::from(Span::styled("┃ ", t.accent())),
            Line::from(Span::styled("┃ ", t.accent())),
        ])),
        area,
        &mut state,
    );

    let viewport = (inner.height as usize) / (2 + spacing as usize);
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

fn meta_width(entries: &[(usize, &SshHost)]) -> u16 {
    let widest = entries
        .iter()
        .map(|(_, host)| {
            let port = if host.has_custom_port() { host.port.to_string().len() + 3 } else { 0 };
            let key = key_name(host).chars().count();
            port.max(key)
        })
        .max()
        .unwrap_or(0);

    (widest as u16).min(14)
}

fn key_name(host: &SshHost) -> &str {
    if !host.has_identity_file() {
        return "";
    }
    host.identity_file.rsplit('/').next().unwrap_or("")
}

/// One host as a two line card: what you type to connect, then where it
/// actually goes, with the details worth scanning pushed to the right.
fn card<'a>(app: &AppService, is_selected: bool, host: &SshHost, text_width: usize) -> Vec<Cell<'a>> {
    let t = &app.theme;

    let alias_style = if is_selected {
        t.bold_accent()
    } else {
        t.base().add_modifier(Modifier::BOLD)
    };

    let target = if host.user.is_empty() {
        host.display_host().to_string()
    } else {
        format!("{}@{}", host.user, host.display_host())
    };

    let text = Text::from(vec![
        Line::from(Span::styled(ellipsize(&host.alias, text_width), alias_style)),
        Line::from(Span::styled(ellipsize(&target, text_width), t.muted())),
    ]);

    let port = if host.has_custom_port() {
        Line::from(Span::styled(format!(" :{} ", host.port), t.pill(&t.accent_secondary)))
    } else {
        Line::from("")
    };

    let key = Span::styled(key_name(host).to_string(), t.muted());

    vec![
        Cell::from(text),
        Cell::from(Text::from(vec![port, Line::from(key)]).alignment(Alignment::Right)),
    ]
}

/// An empty panel says what is missing and what to press, centred so it reads
/// as a deliberate state rather than as a panel that failed to draw.
fn draw_placeholder(frame: &mut Frame, app: &AppService, block: Block, area: Rect, headline: &str, hint: &str) {
    let t = &app.theme;
    let inner = block.inner(area);

    let mut lines = vec![Line::from(""); (inner.height / 2).saturating_sub(2) as usize];
    lines.push(Line::from(Span::styled(
        headline.to_string(),
        t.base().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(hint.to_string(), t.muted())));

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).block(block),
        area,
    );
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
        draw_placeholder(frame, app, block, area, "Nothing selected", "Pick a host on the left");
        return;
    };

    let value = t.base();
    let dim = t.muted();
    let width = block.inner(area).width as usize;

    let port_display = host.port.to_string();
    let port_style = if host.has_custom_port() { value } else { dim };

    let user_display: &str = if host.user.is_empty() { "(default)" } else { &host.user };
    let user_style = if host.user.is_empty() { dim } else { value };

    let key_display: &str = if host.has_identity_file() { &host.identity_file } else { "(default)" };
    let key_style = if host.has_identity_file() { value } else { dim };

    let mut lines = vec![Line::from(vec![
        Span::styled("▏ ", t.accent()),
        Span::styled(host.alias.as_str(), t.title()),
    ])];

    if app.show_command {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            ellipsize(&format!(" $ {} ", host.as_ssh_command()), width),
            t.input(),
        )));
    }

    lines.push(Line::from(""));
    lines.extend(detail_row("HostName ", host.display_host(), dim, value, width));
    lines.extend(detail_row("Port     ", &port_display, dim, port_style, width));
    lines.extend(detail_row("User     ", user_display, dim, user_style, width));
    lines.extend(detail_row("Identity ", key_display, dim, key_style, width));

    if host.has_extra_options() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Extra options", t.bold_accent_secondary())));
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

/// Label and value share a line while they fit, and the value drops to its
/// own indented line when they do not, which reads better than a wrap.
fn detail_row<'a>(
    label: &'a str,
    value: &'a str,
    label_style: Style,
    value_style: Style,
    width: usize,
) -> Vec<Line<'a>> {
    if label.chars().count() + value.chars().count() <= width {
        return vec![Line::from(vec![
            Span::styled(label, label_style),
            Span::styled(value, value_style),
        ])];
    }

    vec![
        Line::from(Span::styled(label.trim_end(), label_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(ellipsize(value, width.saturating_sub(2)), value_style),
        ]),
    ]
}

pub fn draw_status_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;

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
