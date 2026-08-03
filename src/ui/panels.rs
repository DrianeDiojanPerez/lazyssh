use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
    Frame,
};

use crate::models::{Mode, Reachability, SshHost};
use crate::services::AppService;

pub fn draw_search_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;
    let is_focused = app.mode == Mode::Search;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_focused { t.border_focused() } else { t.border() })
        .title(Span::styled(" Filter ", if is_focused { t.title() } else { t.muted() }))
        .padding(Padding::new(1, 1, 0, 0))
        .style(t.base());

    let mut spans = vec![Span::styled("/ ", t.accent())];

    if app.search_query.is_empty() {
        if is_focused {
            spans.push(Span::styled("▎", Style::default().fg(t.input_cursor.to_color())));
        }
        spans.push(Span::styled(" type to narrow the list", t.muted()));
    } else {
        spans.push(Span::styled(&app.search_query, t.base().add_modifier(Modifier::BOLD)));
        if is_focused {
            spans.push(Span::styled("▎", Style::default().fg(t.input_cursor.to_color())));
        }
        spans.push(Span::styled(
            format!("   {} of {}", app.visible_hosts().len(), app.host_count()),
            t.muted(),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

/// The panel the cards live in. Built in one place because mouse hit testing
/// needs the very same borders and padding the renderer uses.
fn list_block(app: &AppService) -> Block<'static> {
    let t = &app.theme;
    let is_focused = matches!(app.mode, Mode::Normal | Mode::Search);

    let entries = app.visible_hosts();
    let title = if app.has_filter() {
        format!(" Hosts {}/{}  '{}' ", entries.len(), app.host_count(), ellipsize(&app.search_query, 12))
    } else {
        format!(" Hosts {} ", app.host_count())
    };

    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_focused { t.border_focused() } else { t.border() })
        .title(Span::styled(title, if is_focused { t.title() } else { t.muted() }))
        .padding(Padding::new(1, 1, 1, 0))
        .style(t.base())
}

/// Where the cards land inside the panel. Every card is the same height, so
/// the row a click lands on maps straight back to a host.
pub struct Cards {
    pub top: u16,
    pub height: u16,
    pub visible: usize,
    pub offset: usize,
}

pub fn cards(app: &AppService, area: Rect) -> Cards {
    let inner = list_block(app).inner(area);

    // a card is its own little box: two edges around the two lines it holds
    let height = 4;
    let visible = (inner.height / height).max(1) as usize;

    Cards {
        top: inner.y,
        height,
        visible,
        // INFO: the table starts every frame from a fresh state, so it scrolls
        // just far enough to reach the selected card and no further
        offset: app.cursor.saturating_sub(visible.saturating_sub(1)),
    }
}

pub fn host_at(app: &AppService, area: Rect, column: u16, row: u16) -> Option<usize> {
    if column < area.x || column >= area.right() || row < area.y || row >= area.bottom() {
        return None;
    }

    let cards = cards(app, area);
    let slot = (row.checked_sub(cards.top)? / cards.height) as usize;
    if slot >= cards.visible {
        return None;
    }

    let index = cards.offset + slot;
    (index < app.visible_hosts().len()).then_some(index)
}

pub fn draw_host_list(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;
    let entries = app.visible_hosts();

    let block = list_block(app);
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

    let layout = cards(app, area);

    let rows: Vec<Row> = entries
        .iter()
        .enumerate()
        .map(|(row, (_, host))| card(app, row == app.cursor, host, inner.width as usize))
        .map(|lines| Row::new(vec![Cell::from(Text::from(lines))]).height(layout.height))
        .collect();

    // INFO: TableState owns the scroll offset, which is what keeps the
    // selected host on screen once the list is taller than the panel
    let mut state = TableState::default().with_selected(Some(app.cursor));

    frame.render_stateful_widget(
        Table::new(rows, [Constraint::Percentage(100)]).block(block).column_spacing(0),
        area,
        &mut state,
    );

    if entries.len() > layout.visible {
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

fn key_name(host: &SshHost) -> &str {
    if !host.has_identity_file() {
        return "";
    }
    host.identity_file.rsplit('/').next().unwrap_or("")
}

fn card<'a>(app: &AppService, is_selected: bool, host: &SshHost, width: usize) -> Vec<Line<'a>> {
    let t = &app.theme;

    // INFO: every card keeps the same rounded frame, so selection shows in the
    // colour it is drawn in rather than in a different shape
    let glyphs = ["╭", "─", "╮", "│ ", " │", "╰", "╯"];
    let (edge, alias_style) = if is_selected {
        (t.accent(), t.bold_accent())
    } else {
        (t.border(), t.base().add_modifier(Modifier::BOLD))
    };

    // "│ " on the left and " │" on the right leave this much for the text
    let room = width.saturating_sub(4);

    let target = if host.user.is_empty() {
        host.display_host().to_string()
    } else {
        format!("{}@{}", host.user, host.display_host())
    };

    let port = host
        .has_custom_port()
        .then(|| Span::styled(format!(" :{} ", host.port), t.pill(&t.accent_secondary)));
    let key = key_name(host);

    let (lamp, lamp_style) = match app.probes.status(&host.alias) {
        Reachability::Online => ("●", t.success_dot()),
        Reachability::Offline => ("●", t.error()),
        Reachability::Checking => ("◌", t.muted()),
        Reachability::Unknown => ("○", t.muted()),
    };

    let mut head: Vec<Span> = Vec::new();
    if let Some(port) = port {
        head.push(port);
        head.push(Span::raw(" "));
    }
    head.push(Span::styled(lamp, lamp_style));

    vec![
        Line::from(Span::styled(
            format!("{}{}{}", glyphs[0], glyphs[1].repeat(width.saturating_sub(2)), glyphs[2]),
            edge,
        )),
        inside_many(glyphs, edge, room, &host.alias, alias_style, head),
        inside(
            glyphs,
            edge,
            room,
            &target,
            t.muted(),
            (!key.is_empty()).then(|| Span::styled(key.to_string(), t.muted())),
        ),
        Line::from(Span::styled(
            format!("{}{}{}", glyphs[5], glyphs[1].repeat(width.saturating_sub(2)), glyphs[6]),
            edge,
        )),
    ]
}

/// A line inside a card: something on the left, something optional pushed hard
/// right, and the card's own sides around them. The left gives way first when
/// the two of them will not fit.
fn inside<'a>(
    glyphs: [&'a str; 7],
    edge: Style,
    room: usize,
    text: &str,
    style: Style,
    meta: Option<Span<'a>>,
) -> Line<'a> {
    inside_many(glyphs, edge, room, text, style, meta.into_iter().collect())
}

fn inside_many<'a>(
    glyphs: [&'a str; 7],
    edge: Style,
    room: usize,
    text: &str,
    style: Style,
    meta: Vec<Span<'a>>,
) -> Line<'a> {
    let taken = match meta.is_empty() {
        true => 0,
        false => meta.iter().map(|m| m.content.chars().count()).sum::<usize>() + 1,
    };
    let text = ellipsize(text, room.saturating_sub(taken));
    let used = text.chars().count() + taken.saturating_sub(1);

    let mut spans = vec![
        Span::styled(glyphs[3], edge),
        Span::styled(text, style),
        Span::raw(" ".repeat(room.saturating_sub(used))),
    ];
    spans.extend(meta);
    spans.push(Span::styled(glyphs[4], edge));

    Line::from(spans)
}

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

    let meta = meta_spans(app, area.width);
    let meta_width: usize = meta.iter().map(|s| s.content.chars().count()).sum();

    let mut spans = vec![mode_chip(app), Span::styled("  ", t.status_bar())];
    let mut used: usize = spans.iter().map(|s| s.content.chars().count()).sum();

    for (_, _, key, label, _) in hint_layout(app, hint_room(app, area.width)) {
        spans.push(Span::styled(key, t.bold_accent()));
        spans.push(Span::styled(format!(" {}", label), t.muted()));
        spans.push(Span::styled(" · ", t.border()));
        used += key.chars().count() + label.chars().count() + 4;
    }
    spans.pop();
    used = used.saturating_sub(3);

    let gap = (area.width as usize).saturating_sub(used + meta_width);
    spans.push(Span::styled(" ".repeat(gap), t.status_bar()));
    spans.extend(meta);

    frame.render_widget(Paragraph::new(Line::from(spans)).style(t.status_bar()), area);
}

/// Where the config is, how much is in it and what it is wearing. Segments are
/// dropped from the end as the bar narrows, so the path outlives the version.
fn meta_spans<'a>(app: &AppService, width: u16) -> Vec<Span<'a>> {
    let t = &app.theme;
    let transparency = if t.transparent { " [T]" } else { "" };

    let segments = [
        (app.config_path_display(), t.accent_secondary()),
        (format!("{} hosts", app.host_count()), t.muted()),
        (format!("{}{}", t.name, transparency), t.muted()),
        (format!("v{}", env!("CARGO_PKG_VERSION")), t.border()),
    ];

    let mut spans: Vec<Span> = Vec::new();
    let mut used = 1;

    for (text, style) in segments {
        let separator = if spans.is_empty() { 0 } else { 3 };
        if used + separator + text.chars().count() > (width / 2) as usize {
            break;
        }
        used += separator + text.chars().count();

        if separator > 0 {
            spans.push(Span::styled(" · ", t.border()));
        }
        spans.push(Span::styled(text, style));
    }

    spans.push(Span::styled(" ", t.status_bar()));
    spans
}

fn hint_room(app: &AppService, width: u16) -> u16 {
    let meta: usize = meta_spans(app, width).iter().map(|s| s.content.chars().count()).sum();
    width.saturating_sub(meta as u16 + 1)
}

pub fn hint_at(app: &AppService, area: Rect, column: u16, row: u16) -> Option<KeyCode> {
    if row != area.y {
        return None;
    }

    hint_layout(app, hint_room(app, area.width))
        .into_iter()
        .find(|(x, width, _, _, _)| column >= *x && column < x + width)
        .map(|(_, _, _, _, code)| code)
}

/// Lays the hints out left to right, dropping the ones that will not fit. The
/// renderer and the mouse both read this, so a click always lands on the hint
/// that is actually printed there.
fn hint_layout(
    app: &AppService,
    width: u16,
) -> Vec<(u16, u16, &'static str, &'static str, KeyCode)> {
    let mut placed = Vec::new();
    let mut x: u16 = mode_chip(app).content.chars().count() as u16 + 2;

    // INFO: the way out of the current mode is worth more than the rest, so
    // its width is reserved before the others are laid out
    let escape = escape_hint(app);
    let reserved = escape.0.chars().count() as u16 + escape.1.chars().count() as u16 + 4;

    for (key, label, code) in hints_for(app) {
        let span = key.chars().count() as u16 + label.chars().count() as u16 + 1;
        if x + span + 3 + reserved > width {
            break;
        }

        placed.push((x, span, *key, *label, *code));
        x += span + 3;
    }

    let span = escape.0.chars().count() as u16 + escape.1.chars().count() as u16 + 1;
    placed.push((x, span, escape.0, escape.1, escape.2));
    placed
}

fn mode_chip<'a>(app: &AppService) -> Span<'a> {
    if app.is_session_focused() {
        return Span::styled(" SESSION ", app.theme.pill(&app.theme.success));
    }

    let label = match &app.mode {
        Mode::Normal => "NORMAL",
        Mode::Search => "SEARCH",
        Mode::AddHost => "ADD",
        Mode::EditHost(_) => "EDIT",
        Mode::ConfirmDelete(_) => "DELETE",
        Mode::SelectTheme => "THEME",
        Mode::ChooseLaunch => "CONNECT",
        Mode::Settings => "SETTINGS",
        Mode::Help => "HELP",
    };

    Span::styled(format!(" {} ", label), app.theme.selected())
}

fn escape_hint(app: &AppService) -> (&'static str, &'static str, KeyCode) {
    if app.is_session_focused() {
        return ("C-b", "sidebar", KeyCode::Null);
    }

    match &app.mode {
        Mode::Normal => ("?", "help", KeyCode::Char('?')),
        Mode::Search => ("Esc", "clear", KeyCode::Esc),
        Mode::AddHost | Mode::EditHost(_) => ("Esc", "cancel", KeyCode::Esc),
        Mode::ConfirmDelete(_) => ("Esc", "keep", KeyCode::Esc),
        Mode::SelectTheme | Mode::Settings | Mode::Help => ("Esc", "close", KeyCode::Esc),
        Mode::ChooseLaunch => ("Esc", "cancel", KeyCode::Esc),
    }
}

/// The hints that matter in the current mode, most useful first, so the
/// status bar degrades sensibly on a narrow terminal.
fn hints_for(app: &AppService) -> &'static [(&'static str, &'static str, KeyCode)] {
    if app.is_session_focused() {
        return &[("keys", "go to the session", KeyCode::Null)];
    }

    match &app.mode {
        Mode::Normal => &[
            ("↵", "connect", KeyCode::Enter),
            ("a", "add", KeyCode::Char('a')),
            ("e", "edit", KeyCode::Char('e')),
            ("d", "delete", KeyCode::Char('d')),
            ("/", "filter", KeyCode::Char('/')),
            ("q", "quit", KeyCode::Char('q')),
            ("↑↓", "move", KeyCode::Null),
            ("c", "command", KeyCode::Char('c')),
            ("r", "reload", KeyCode::Char('r')),
            ("w", "close tab", KeyCode::Char('w')),
            ("s", "settings", KeyCode::Char('s')),
            ("t", "theme", KeyCode::Char('t')),
            ("T", "transparency", KeyCode::Char('T')),
        ],
        Mode::Search => &[
            ("type", "to filter", KeyCode::Null),
            ("↵", "keep filter", KeyCode::Enter),
            ("↑↓", "move", KeyCode::Null),
        ],
        Mode::AddHost | Mode::EditHost(_) => &[
            ("Tab", "next field", KeyCode::Tab),
            ("S-Tab", "previous", KeyCode::BackTab),
            ("↵", "save", KeyCode::Enter),
        ],
        Mode::ConfirmDelete(_) => &[("y", "delete", KeyCode::Char('y'))],
        Mode::SelectTheme => &[
            ("↑↓", "browse", KeyCode::Null),
            ("↵", "apply", KeyCode::Enter),
        ],
        Mode::ChooseLaunch => &[
            ("t", "in a tab", KeyCode::Char('t')),
            ("f", "whole terminal", KeyCode::Char('f')),
        ],
        Mode::Settings => &[
            ("↑↓", "browse", KeyCode::Null),
            ("↵", "choose", KeyCode::Enter),
        ],
        Mode::Help => &[],
    }
}
