use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState,
    },
    Frame,
};

use crate::models::{Mode, Reachability, Rgb, SshHost};
use crate::services::AppService;

/// Colour without a background, for anything drawn on top of a surface that
/// has already been filled: the theme's own styles carry the page background
/// and would punch a hole through the card they sit in.
fn ink(color: &Rgb) -> Style {
    Style::default().fg(color.to_color())
}

/// The line down the far side of the sidebar, which is all that separates it
/// from the pane beside it now that neither of them is boxed.
pub fn draw_sidebar_edge(frame: &mut Frame, app: &AppService, area: Rect) {
    let rule = Rect { x: area.right(), y: area.y, width: 1, height: area.height };
    let line = Line::from(Span::styled("│", ink(&app.theme.border)));

    frame.render_widget(Paragraph::new(vec![line; area.height as usize]), rule);
}

pub fn draw_search_bar(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;
    let is_focused = app.mode == Mode::Search;

    // a column of air so the box does not run straight into the panel edge
    let area = Rect { width: area.width.saturating_sub(1), ..area };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(ink(if is_focused { &t.border_focused } else { &t.border }))
        .title(Span::styled(
            " Filter ",
            if is_focused { ink(&t.accent).add_modifier(Modifier::BOLD) } else { ink(&t.muted) },
        ))
        .padding(Padding::new(1, 1, 0, 0))
        .style(t.input());

    let mut spans = vec![Span::styled("/ ", ink(&t.accent))];

    if app.search_query.is_empty() {
        if is_focused {
            spans.push(Span::styled("▎", ink(&t.input_cursor)));
        }
        spans.push(Span::styled(" type to narrow the list", ink(&t.muted)));
    } else {
        spans.push(Span::styled(&app.search_query, ink(&t.fg).add_modifier(Modifier::BOLD)));
        if is_focused {
            spans.push(Span::styled("▎", ink(&t.input_cursor)));
        }
        spans.push(Span::styled(
            format!("   {} of {}", app.visible_hosts().len(), app.host_count()),
            ink(&t.muted),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

/// The surface the cards sit on. Built in one place because mouse hit testing
/// needs the very same padding the renderer uses. The three rows it keeps free
/// at the top are the header and the air around it.
fn list_block(app: &AppService) -> Block<'static> {
    Block::default().padding(Padding::new(1, 1, 3, 0)).style(app.theme.input())
}

fn list_header(app: &AppService) -> String {
    if app.has_filter() {
        return format!(
            "Hosts {}/{}  '{}'",
            app.visible_hosts().len(),
            app.host_count(),
            ellipsize(&app.search_query, 12)
        );
    }

    format!("Hosts {}", app.host_count())
}

/// The count, then a rule out to the far side of the panel.
fn draw_list_header(frame: &mut Frame, app: &AppService, area: Rect) {
    let t = &app.theme;
    let row = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: 1 };

    let title = list_header(app);
    let rule = (row.width as usize).saturating_sub(title.chars().count() + 1);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(title, ink(&t.muted)),
            Span::raw(" "),
            Span::styled("─".repeat(rule), ink(&t.border)),
        ])),
        row,
    );
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

    // the two lines a host is written on, and a blank one holding it apart
    // from the next
    let height = 3;
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
        draw_list_header(frame, app, area);
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

    draw_list_header(frame, app, area);

    if entries.len() > layout.visible {
        let mut scrollbar_state = ScrollbarState::new(entries.len()).position(app.cursor);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(ink(&t.border)),
            area.inner(&Margin { vertical: 2, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

fn lamp_for(app: &AppService, host: &SshHost) -> (&'static str, Style) {
    let t = &app.theme;

    match app.probes.status(&host.alias) {
        Reachability::Online => ("●", ink(&t.success)),
        Reachability::Offline => ("●", ink(&t.error)),
        Reachability::Checking => ("◌", ink(&t.muted)),
        Reachability::Unknown => ("○", ink(&t.muted)),
    }
}

fn key_name(host: &SshHost) -> &str {
    if !host.has_identity_file() {
        return "";
    }
    host.identity_file.rsplit('/').next().unwrap_or("")
}

/// A host, written flat on two lines. Nothing is drawn around it, so the one
/// under the cursor is marked by the bar down its side and the surface it sits
/// on rather than by a shape the others do not have.
fn card<'a>(app: &AppService, is_selected: bool, host: &SshHost, width: usize) -> Vec<Line<'a>> {
    let t = &app.theme;

    let surface = match is_selected {
        true => Style::default().bg(t.selected_bg.to_color()),
        false => Style::default(),
    };
    let bar = match is_selected {
        true => Span::styled("▎ ", ink(&t.accent)),
        false => Span::raw("  "),
    };

    // the bar on the left and a column of air on the right
    let room = width.saturating_sub(3);

    let target = if host.user.is_empty() {
        host.display_host().to_string()
    } else {
        format!("{}@{}", host.user, host.display_host())
    };

    // INFO: the chip has to change surface on the selected row, or it is the
    // same colour as the fill behind it and disappears
    let chip = Style::default().fg(t.accent_secondary.to_color()).bg(match is_selected {
        true => t.background(),
        false => t.selected_bg.to_color(),
    });

    let key = key_name(host);
    let (lamp, lamp_style) = lamp_for(app, host);

    let mut head: Vec<Span> = Vec::new();
    if host.has_custom_port() {
        head.push(Span::styled(format!(" :{} ", host.port), chip));
        head.push(Span::raw(" "));
    }
    head.push(Span::styled(lamp, lamp_style));

    vec![
        row_line(bar, room, &host.alias, ink(&t.fg).add_modifier(Modifier::BOLD), head)
            .style(surface),
        row_line(
            Span::raw("  "),
            room,
            &target,
            ink(if is_selected { &t.accent } else { &t.muted }),
            (!key.is_empty())
                .then(|| Span::styled(key.to_string(), ink(&t.muted)))
                .into_iter()
                .collect(),
        )
        .style(surface),
        Line::from(""),
    ]
}

/// A line of a card: the bar down its side, something on the left, and
/// something optional pushed hard right. The left gives way first when the two
/// of them will not fit.
fn row_line<'a>(
    bar: Span<'a>,
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
        bar,
        Span::styled(text, style),
        Span::raw(" ".repeat(room.saturating_sub(used))),
    ];
    spans.extend(meta);
    spans.push(Span::raw(" "));

    Line::from(spans)
}

fn draw_placeholder(frame: &mut Frame, app: &AppService, block: Block, area: Rect, headline: &str, hint: &str) {
    let t = &app.theme;
    let inner = block.inner(area);

    let mut lines = vec![Line::from(""); (inner.height / 2).saturating_sub(2) as usize];
    lines.push(Line::from(Span::styled(
        headline.to_string(),
        ink(&t.fg).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(hint.to_string(), ink(&t.muted))));

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).block(block),
        area,
    );
}

pub fn draw_detail_panel(frame: &mut Frame, app: &AppService, area: Rect) {
    let Some(host) = app.selected_host() else {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0)).style(app.theme.base());
        draw_placeholder(frame, app, block, area, "Nothing selected", "Pick a host on the left");
        return;
    };

    let inner = area.inner(&Margin { horizontal: 2, vertical: 1 });
    if inner.width < 12 || inner.height < 3 {
        return;
    }

    // INFO: the card is the first thing to go on a short pane, since the block
    // under it is what the panel is actually for
    let wanted = if app.show_command { command_height(app, host) } else { 0 };
    let command = if inner.height >= wanted + 8 { wanted } else { 0 };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(command),
            Constraint::Length(if command > 0 { 1 } else { 0 }),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let width = inner.width as usize;

    frame.render_widget(Paragraph::new(breadcrumb(app, host, width)), rows[0]);
    if command > 0 {
        draw_command_card(frame, app, host, rows[2]);
    }
    draw_config_card(frame, app, host, rows[4]);
    frame.render_widget(Paragraph::new(detail_hints(app, width)), rows[6]);
}

/// Where the host sits: the file it was read out of, then the host itself,
/// with the same lamp its card in the list wears. The host is the point of the
/// line, so the file is what gives way when the two will not fit.
fn breadcrumb<'a>(app: &AppService, host: &SshHost, width: usize) -> Line<'a> {
    let t = &app.theme;
    let (lamp, lamp_style) = lamp_for(app, host);
    let path = app.config_path_display();

    let mut spans = vec![Span::styled(lamp, lamp_style), Span::styled(" ", t.base())];

    if 2 + path.chars().count() + 3 + host.alias.chars().count() <= width {
        spans.push(Span::styled(path, t.muted()));
        spans.push(Span::styled(" › ", t.border()));
    }

    spans.push(Span::styled(
        ellipsize(&host.alias, width.saturating_sub(2)),
        t.title(),
    ));
    Line::from(spans)
}

/// The surface both cards are drawn on, a shade off the panel behind them.
fn card_block(app: &AppService) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(ink(&app.theme.border))
        .padding(Padding::new(1, 1, 0, 0))
        .style(app.theme.input())
}

fn command_height(app: &AppService, host: &SshHost) -> u16 {
    match reach_line(app, host) {
        Some(_) => 4,
        None => 3,
    }
}

/// What the probe found, said in words under the command. Nothing is said
/// while the host has not been asked yet.
fn reach_line(app: &AppService, host: &SshHost) -> Option<(&'static str, String, Style)> {
    let t = &app.theme;

    match app.probes.status(&host.alias) {
        Reachability::Online => Some(("✓", format!("reachable at {}", target(host)), ink(&t.success))),
        Reachability::Offline => Some((
            "✗",
            format!("no answer from {}:{}", host.display_host(), host.port),
            ink(&t.error),
        )),
        Reachability::Checking => {
            Some(("◌", format!("checking {}…", host.display_host()), ink(&t.warning)))
        }
        Reachability::Unknown => None,
    }
}

fn target(host: &SshHost) -> String {
    if host.user.is_empty() {
        return host.display_host().to_string();
    }
    format!("{}@{}", host.user, host.display_host())
}

fn draw_command_card(frame: &mut Frame, app: &AppService, host: &SshHost, area: Rect) {
    let t = &app.theme;
    let block = card_block(app);
    let width = block.inner(area).width as usize;

    let mut lines = vec![Line::from(Span::styled(
        ellipsize(&format!("$ {}", host.as_ssh_command()), width),
        ink(&t.success),
    ))];

    if let Some((mark, text, style)) = reach_line(app, host) {
        lines.push(Line::from(vec![
            Span::styled(mark, style),
            Span::raw(" "),
            Span::styled(ellipsize(&text, width.saturating_sub(2)), ink(&t.muted)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_config_card(frame: &mut Frame, app: &AppService, host: &SshHost, area: Rect) {
    let block = card_block(app);
    let lines = config_lines(app, host, block.inner(area).width as usize);

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// The host as it reads in the file, numbered down the side. The keywords are
/// coloured by where they came from: the block header, the four fields the
/// form knows about, and whatever else was typed in by hand.
fn config_lines<'a>(app: &AppService, host: &SshHost, width: usize) -> Vec<Line<'a>> {
    let t = &app.theme;
    let value = ink(&t.fg);
    let missing = ink(&t.muted);

    let field = |name: &str, given: bool| (name.to_string(), ink(&t.accent), if given { value } else { missing });
    let unset = || "(default)".to_string();

    let mut entries = vec![
        (host.alias.clone(), ("Host".to_string(), ink(&t.accent_secondary), value)),
        (host.display_host().to_string(), field("HostName", true)),
        (host.port.to_string(), field("Port", host.has_custom_port())),
        (
            if host.user.is_empty() { unset() } else { host.user.clone() },
            field("User", !host.user.is_empty()),
        ),
        (
            if host.has_identity_file() { host.identity_file.clone() } else { unset() },
            field("IdentityFile", host.has_identity_file()),
        ),
    ];

    entries.extend(
        host.extra_options
            .iter()
            .map(|(k, v)| (v.clone(), (k.clone(), ink(&t.warning), value))),
    );

    let gutter = entries.len().to_string().chars().count();

    entries
        .into_iter()
        .enumerate()
        .map(|(row, (text, (keyword, keyword_style, text_style)))| {
            let room = width.saturating_sub(gutter + 3 + keyword.chars().count() + 1);
            Line::from(vec![
                Span::styled(format!("{:>1$}", row + 1, gutter), ink(&t.border)),
                Span::styled(" │ ", ink(&t.border)),
                Span::styled(keyword, keyword_style),
                Span::raw(" "),
                Span::styled(ellipsize(&text, room), text_style),
            ])
        })
        .collect()
}

fn detail_hints<'a>(app: &AppService, width: usize) -> Line<'a> {
    let t = &app.theme;
    let mut spans = Vec::new();
    let mut used = 0;

    for (key, label) in [("↵", "connect"), ("e", "edit"), ("d", "delete")] {
        let gap = if spans.is_empty() { 0 } else { 3 };
        let room = key.chars().count() + label.chars().count() + 1;
        if used + gap + room > width {
            break;
        }
        used += gap + room;

        if gap > 0 {
            spans.push(Span::styled("   ", t.base()));
        }
        spans.push(Span::styled(key, t.bold_accent()));
        spans.push(Span::styled(format!(" {}", label), t.muted()));
    }

    Line::from(spans)
}

fn ellipsize(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit.saturating_sub(1)).chain("…".chars()).collect()
}
