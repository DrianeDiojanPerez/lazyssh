use std::time::Duration;

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

use crate::models::{FormField, Setting};
use crate::services::AppService;

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

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// What is on screen while ssh is off resolving, connecting and agreeing on
/// keys, whether the session is being made in a tab or the terminal is about to
/// be handed over. It sits over what was already there rather than taking the
/// screen, so there is still something to read while the wait goes on. The mark
/// is worked out from the clock, so nothing has to be stepped along to turn it.
pub fn draw_connecting(
    frame: &mut Frame,
    app: &AppService,
    alias: &str,
    waited: Duration,
    body: Rect,
) {
    let t = &app.theme;

    let target = app
        .host_named(alias)
        .map(|host| host.display_host().to_string())
        .unwrap_or_else(|| alias.to_string());

    let said = format!("Connecting to {}…", target);
    let area = centered(said.chars().count() as u16 + 12, 5, body);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.accent())
        .title(Span::styled(format!(" {} ", alias), t.title()))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
        .style(t.base());

    let turn = (waited.as_millis() / 100) as usize;
    let line = Line::from(vec![
        Span::styled(SPINNER[turn % SPINNER.len()], Style::default().fg(t.warning.to_color())),
        Span::styled(format!(" {}", said), t.muted()),
    ]);

    frame.render_widget(
        Paragraph::new(line).alignment(Alignment::Center).block(block),
        area,
    );
}

/// Everything the form draws, worked out up front so a click can be matched
/// against the very rows and buttons that end up on screen.
pub struct FormLayout {
    pub area: Rect,
    pub inner: Rect,
    pub scroll: u16,
    pub save: Rect,
    pub cancel: Rect,
}

const SAVE: &str = " Save ";
const CANCEL: &str = " Cancel ";

/// How many rows a field takes: its label, its value and a blank row under it,
/// except a list, which gets a row for every line it holds.
fn field_rows(app: &AppService, field: &FormField) -> usize {
    match field.is_multiline() {
        true => 2 + option_count(app),
        false => 3,
    }
}

fn option_count(app: &AppService) -> usize {
    app.form_value(&FormField::Options).split('\n').count()
}

fn rows_above(app: &AppService, field: &FormField) -> usize {
    FormField::all()
        .iter()
        .take_while(|above| *above != field)
        .map(|above| field_rows(app, above))
        .sum()
}

fn all_rows(app: &AppService) -> usize {
    FormField::all().iter().map(|field| field_rows(app, field)).sum()
}

pub fn form_layout(app: &AppService, body: Rect) -> FormLayout {
    let content_height = all_rows(app) as u16 + if app.form_error.is_some() { 2 } else { 1 };
    let area = centered(58, content_height + 3, body);
    let inner = form_block("").inner(area);

    // INFO: on a terminal too short for the whole form, scroll just enough to
    // keep the field being edited on screen
    let active_row = rows_above(app, &app.form_field) + field_rows(app, &app.form_field) - 1;
    let scroll = active_row.saturating_sub(inner.height as usize) as u16;

    let footer = inner.y + all_rows(app) as u16
        + u16::from(app.form_error.is_some())
        - scroll;
    let buttons = SAVE.len() as u16 + CANCEL.len() as u16 + 2;

    FormLayout {
        area,
        inner,
        scroll,
        save: Rect {
            x: inner.right().saturating_sub(buttons),
            y: footer,
            width: SAVE.len() as u16,
            height: 1,
        },
        cancel: Rect {
            x: inner.right().saturating_sub(CANCEL.len() as u16),
            y: footer,
            width: CANCEL.len() as u16,
            height: 1,
        },
    }
}

/// The field a point falls in. The label, the input and the gap under it all
/// belong to the same field, so an approximate click still lands.
pub fn field_at(app: &AppService, body: Rect, column: u16, row: u16) -> Option<FormField> {
    let layout = form_layout(app, body);
    if column < layout.inner.x || column >= layout.inner.right() {
        return None;
    }

    let offset = (row.checked_sub(layout.inner.y)? + layout.scroll) as usize;

    let mut top = 0;
    for field in FormField::all() {
        top += field_rows(app, &field);
        if offset < top {
            return Some(field);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormButton {
    Save,
    Cancel,
}

pub fn form_button_at(app: &AppService, body: Rect, column: u16, row: u16) -> Option<FormButton> {
    let layout = form_layout(app, body);

    if hits(layout.save, column, row) {
        return Some(FormButton::Save);
    }
    if hits(layout.cancel, column, row) {
        return Some(FormButton::Cancel);
    }
    None
}

fn hits(rect: Rect, column: u16, row: u16) -> bool {
    row == rect.y && column >= rect.x && column < rect.right()
}

fn form_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::raw(title))
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
}

/// The end of a value that is too long for its box, which is the end that is
/// being typed: there is no way to move the cursor back through it.
fn tail(value: &str, room: usize) -> String {
    let count = value.chars().count();
    if count <= room {
        return value.to_string();
    }

    "…".chars().chain(value.chars().skip(count - room + 1)).collect()
}

/// The options, one to a line and numbered down the side the way they read in
/// the file. Everything up to the first space is the name of the option, which
/// is enough to colour a line that is only half typed.
fn option_lines<'a>(app: &AppService, is_active: bool, width: usize) -> Vec<Line<'a>> {
    let t = &app.theme;
    let text = app.form_value(&FormField::Options);
    let entries: Vec<&str> = text.split('\n').collect();
    let gutter = entries.len().to_string().chars().count();

    entries
        .iter()
        .enumerate()
        .map(|(row, entry)| {
            let last = row + 1 == entries.len();
            let shown = tail(entry, width.saturating_sub(gutter + 7));
            let (name, value) = match shown.split_once(' ') {
                Some((name, value)) => (name.to_string(), format!(" {}", value)),
                None => (shown, String::new()),
            };

            let mut spans = vec![
                Span::styled(format!("  {:>1$}", row + 1, gutter), t.border()),
                Span::styled(" │ ", t.border()),
                Span::styled(name, Style::default().fg(t.warning.to_color())),
                Span::styled(value, t.base()),
            ];

            if is_active && last {
                spans.push(Span::styled("▎", Style::default().fg(t.input_cursor.to_color())));
            }

            Line::from(spans)
        })
        .collect()
}

pub fn draw_form(frame: &mut Frame, app: &AppService, title: &str, body: Rect) {
    let t = &app.theme;
    let fields = FormField::all();
    let layout = form_layout(app, body);
    let area = layout.area;

    frame.render_widget(Clear, area);

    let block = form_block("")
        .border_style(t.accent())
        .title(Span::styled(title, t.title()))
        .title_alignment(Alignment::Center)
        .style(t.base());

    let width = layout.inner.width as usize;
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

        if field.is_multiline() {
            lines.extend(option_lines(app, is_active, width));
            lines.push(Line::from(""));
            continue;
        }

        let value = tail(&app.form_value(field), width.saturating_sub(3));
        let cursor = if is_active { "▎" } else { "" };
        let input_style = if is_active { t.input() } else { t.muted() };

        lines.push(Line::from(Span::styled(
            format!("{:<width$}", format!("  {}{}", value, cursor), width = width),
            input_style,
        )));
        lines.push(Line::from(""));
    }

    if let Some(message) = app.form_error.as_ref() {
        lines.push(Line::from(Span::styled(format!("  {}", message), t.bold_error())));
    }

    let hint = if app.is_completing() {
        "  ↑↓ pick a key   Enter use it"
    } else if app.form_field.is_multiline() {
        "  ↵ another line   Tab next field"
    } else {
        "  * required   Tab next field"
    };
    let gap = width.saturating_sub(hint.chars().count() + SAVE.len() + CANCEL.len() + 2);

    lines.push(Line::from(vec![
        Span::styled(hint, t.muted()),
        Span::raw(" ".repeat(gap)),
        Span::styled(SAVE, t.pill(&t.accent)),
        Span::raw("  "),
        Span::styled(CANCEL, t.selected()),
    ]));

    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((layout.scroll, 0)),
        area,
    );

    if app.is_completing() {
        draw_completions(frame, app, body);
    }
}

pub fn completion_rect(app: &AppService, body: Rect) -> Option<Rect> {
    if !app.is_completing() {
        return None;
    }

    let layout = form_layout(app, body);
    let matches = app.identity_matches();

    let width = matches
        .iter()
        .map(|path| path.chars().count() as u16 + 4)
        .max()
        .unwrap_or(0)
        .clamp(24, layout.inner.width);
    let height = (matches.len() as u16 + 2).min(7);

    let row = FormField::all().iter().position(|f| *f == app.form_field).unwrap_or(0);
    let field = layout.inner.y + (row * 3 + 1) as u16 - layout.scroll;

    // INFO: the menu never covers the field it belongs to, so it drops below
    // when there is room, sits above the label when there is not, and stays
    // out of the way entirely when there is room for neither
    let y = if field + 1 + height < layout.area.bottom() {
        field + 1
    } else if field >= layout.area.y + height + 1 {
        field - height - 1
    } else {
        return None;
    };

    Some(Rect { x: layout.inner.x, y, width, height })
}

pub fn suggestion_at(app: &AppService, body: Rect, column: u16, row: u16) -> Option<usize> {
    let area = completion_rect(app, body)?;
    if column <= area.x || column >= area.right() - 1 {
        return None;
    }

    let slot = row.checked_sub(area.y + 1)? as usize;
    let index = completion_offset(app, area) + slot;

    (row < area.bottom() - 1 && index < app.identity_matches().len()).then_some(index)
}

/// INFO: the highlighted key is kept on screen when the list is longer than
/// the menu, the same way the host list scrolls
fn completion_offset(app: &AppService, area: Rect) -> usize {
    let rows = area.height.saturating_sub(2) as usize;
    app.suggestion_cursor.unwrap_or(0).saturating_sub(rows.saturating_sub(1))
}

fn draw_completions(frame: &mut Frame, app: &AppService, body: Rect) {
    let t = &app.theme;
    let Some(area) = completion_rect(app, body) else {
        return;
    };

    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.accent_secondary())
        .title(Span::styled(" keys ", t.bold_accent_secondary()))
        .padding(Padding::horizontal(1))
        .style(t.base());

    let rows = block.inner(area).height as usize;
    let offset = completion_offset(app, area);

    let lines: Vec<Line> = app
        .identity_matches()
        .iter()
        .enumerate()
        .skip(offset)
        .take(rows)
        .map(|(i, path)| {
            let is_picked = app.suggestion_cursor == Some(i);
            Line::from(Span::styled(
                format!("{}{}", if is_picked { "▸ " } else { "  " }, path),
                if is_picked { t.selected() } else { t.muted() },
            ))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

const DELETE: &str = " Delete ";
const KEEP: &str = " Keep ";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeleteButton {
    Delete,
    Keep,
}

fn delete_buttons(body: Rect) -> (Rect, Rect, Rect) {
    let area = centered(52, 8, body);
    let inner = delete_block().inner(area);

    let width = DELETE.len() as u16 + KEEP.len() as u16 + 2;
    let x = inner.x + (inner.width.saturating_sub(width)) / 2;
    let y = inner.y + 4;

    (
        area,
        Rect { x, y, width: DELETE.len() as u16, height: 1 },
        Rect { x: x + DELETE.len() as u16 + 2, y, width: KEEP.len() as u16, height: 1 },
    )
}

pub fn delete_button_at(body: Rect, column: u16, row: u16) -> Option<DeleteButton> {
    let (_, delete, keep) = delete_buttons(body);

    if hits(delete, column, row) {
        return Some(DeleteButton::Delete);
    }
    if hits(keep, column, row) {
        return Some(DeleteButton::Keep);
    }
    None
}

fn delete_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
}

pub fn draw_delete_confirmation(frame: &mut Frame, app: &AppService, index: usize, body: Rect) {
    let t = &app.theme;
    let (area, delete, _) = delete_buttons(body);
    frame.render_widget(Clear, area);

    let alias = app.host_at(index).map(|h| h.alias.as_str()).unwrap_or("?");

    let block = delete_block()
        .border_style(t.error())
        .title(Span::styled(" Delete host ", t.bold_error()))
        .style(t.base());

    let inner = block.inner(area);
    let lead = delete.x.saturating_sub(inner.x) as usize;

    let text = Text::from(vec![
        Line::from(Span::styled(
            format!("Remove '{}' from ~/.ssh/config?", alias),
            t.base().add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Line::from(""),
        Line::from(Span::styled("A timestamped backup is written first.", t.muted()))
            .alignment(Alignment::Center),
        Line::from(""),
        Line::from(vec![
            Span::raw(" ".repeat(lead)),
            Span::styled(DELETE, t.pill(&t.error)),
            Span::raw("  "),
            Span::styled(KEEP, t.selected()),
        ]),
    ]);

    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn theme_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
}

fn theme_area(app: &AppService, body: Rect) -> Rect {
    centered(46, app.available_themes.len() as u16 + 5, body)
}

pub fn theme_at(app: &AppService, body: Rect, column: u16, row: u16) -> Option<usize> {
    let inner = theme_block().inner(theme_area(app, body));
    if column < inner.x || column >= inner.right() {
        return None;
    }

    let index = row.checked_sub(inner.y)? as usize;
    (index < app.available_themes.len()).then_some(index)
}

pub fn draw_theme_selector(frame: &mut Frame, app: &AppService, body: Rect) {
    let t = &app.theme;
    let area = theme_area(app, body);
    frame.render_widget(Clear, area);

    let block = theme_block()
        .border_style(t.accent_secondary())
        .title(Span::styled(" Themes ", t.title()))
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

const TAB: &str = "In a tab";
const FULL: &str = "Whole terminal";

/// Room for the marker, the label and a space on each side.
fn choice_width(label: &str) -> u16 {
    label.chars().count() as u16 + 4
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LaunchButton {
    Tab,
    FullScreen,
}

fn launch_buttons(body: Rect) -> (Rect, Rect, Rect) {
    let area = centered(58, 10, body);
    let inner = choice_block().inner(area);

    let width = choice_width(TAB) + choice_width(FULL) + 2;
    let x = inner.x + inner.width.saturating_sub(width) / 2;
    let y = inner.y + 4;

    (
        area,
        Rect { x, y, width: choice_width(TAB), height: 1 },
        Rect { x: x + choice_width(TAB) + 2, y, width: choice_width(FULL), height: 1 },
    )
}

pub fn launch_button_at(body: Rect, column: u16, row: u16) -> Option<LaunchButton> {
    let (_, tab, full) = launch_buttons(body);

    if hits(tab, column, row) {
        return Some(LaunchButton::Tab);
    }
    if hits(full, column, row) {
        return Some(LaunchButton::FullScreen);
    }
    None
}

fn choice_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_alignment(Alignment::Center)
        .padding(Padding::new(2, 2, 1, 0))
}

pub fn draw_launch_choice(frame: &mut Frame, app: &AppService, body: Rect) {
    let t = &app.theme;
    let (area, tab, _) = launch_buttons(body);
    frame.render_widget(Clear, area);

    let alias = app.selected_host().map(|h| h.alias.as_str()).unwrap_or("?");

    let block = choice_block()
        .border_style(t.accent())
        .title(Span::styled(" Connect ", t.title()))
        .style(t.base());

    let inner = block.inner(area);
    let lead = tab.x.saturating_sub(inner.x) as usize;

    let text = Text::from(vec![
        Line::from(Span::styled(
            format!("How should '{}' open?", alias),
            t.base().add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Line::from(""),
        Line::from(Span::styled("A tab keeps lazyssh beside it", t.muted()))
            .alignment(Alignment::Center),
        Line::from(""),
        Line::from(vec![
            Span::raw(" ".repeat(lead)),
            choice(TAB, app.launch_cursor == 0, t),
            Span::raw("  "),
            choice(FULL, app.launch_cursor == 1, t),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "← → choose   ↵ open   t or f direct   Esc cancel",
            t.muted(),
        ))
        .alignment(Alignment::Center),
    ]);

    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn choice<'a>(label: &'a str, is_picked: bool, t: &crate::models::Theme) -> Span<'a> {
    let text = format!("  {}  ", label);

    match is_picked {
        true => Span::styled(text, t.pill(&t.accent)),
        false => Span::styled(text, t.surface()),
    }
}

fn settings_area(body: Rect) -> Rect {
    centered(56, 16, body)
}

pub fn setting_at(body: Rect, column: u16, row: u16) -> Option<usize> {
    let inner = choice_block().inner(settings_area(body));
    if column < inner.x || column >= inner.right() {
        return None;
    }

    Setting::at_line(row.checked_sub(inner.y)?)
}

fn on_or_off(is_on: bool) -> String {
    match is_on {
        true => "on".to_string(),
        false => "off".to_string(),
    }
}

pub fn draw_settings(frame: &mut Frame, app: &AppService, body: Rect) {
    let t = &app.theme;
    let area = settings_area(body);
    frame.render_widget(Clear, area);

    let block = choice_block()
        .border_style(t.accent_secondary())
        .title(Span::styled(" Settings ", t.title()))
        .style(t.base());

    let width = block.inner(area).width as usize;

    // INFO: rows are placed by the same line numbers the mouse looks them up
    // by, so a click can never land a row away from what it points at
    let mut lines = vec![Line::from(""); 13];
    lines[0] = Line::from(Span::styled("When you connect to a host", t.muted()));
    lines[5] = Line::from(Span::styled("Look", t.muted()));

    for (index, setting) in Setting::all().iter().enumerate() {
        let is_pointed = index == app.settings_cursor;
        let value = match setting {
            Setting::Launch(style) if *style == app.launch_style() => "in use".to_string(),
            Setting::Launch(_) => String::new(),
            Setting::Theme => app.theme.name.clone(),
            Setting::Transparency => on_or_off(app.theme.transparent),
            Setting::TabEdges => on_or_off(app.tab_edges()),
            Setting::TabPanel => on_or_off(app.tab_panel()),
            Setting::Connecting => on_or_off(app.wants_connecting_screen()),
        };

        let label = setting.label();
        let gap = width.saturating_sub(label.chars().count() + value.chars().count() + 3);

        lines[Setting::line(index) as usize] = Line::from(vec![
            Span::styled(if is_pointed { "▸ " } else { "  " }, t.bold_accent()),
            Span::styled(label, if is_pointed { t.bold_accent() } else { t.base() }),
            Span::raw(" ".repeat(gap)),
            Span::styled(value, t.success_dot()),
        ]);
    }

    lines[11] = Line::from(vec![
        Span::styled("  ↑↓", t.bold_accent()),
        Span::styled(" browse  ", t.muted()),
        Span::styled("Enter", t.bold_accent()),
        Span::styled(" choose  ", t.muted()),
        Span::styled("Esc", t.bold_accent()),
        Span::styled(" close", t.muted()),
    ]);

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

struct Section {
    title: &'static str,
    keys: &'static [(&'static str, &'static str)],
}

const COLUMN: usize = 33;
const CAP: usize = 7;

fn sections() -> [[Section; 2]; 2] {
    [
        [
            Section {
                title: "MOVE AROUND",
                keys: &[
                    ("↑ ↓", "up and down the list"),
                    ("g G", "first and last"),
                    ("/", "filter the list"),
                    ("Esc", "clear the filter"),
                ],
            },
            Section {
                title: "CONNECT",
                keys: &[
                    ("↵", "open the host"),
                    ("n", "next tab"),
                    ("w", "close the tab"),
                    ("C-b", "sidebar in and out"),
                ],
            },
        ],
        [
            Section {
                title: "HOSTS",
                keys: &[
                    ("a", "add a host"),
                    ("e", "edit this one"),
                    ("d", "delete this one"),
                    ("r", "reload the config"),
                ],
            },
            Section {
                title: "LOOK AND FEEL",
                keys: &[
                    ("s", "settings"),
                    ("t", "themes"),
                    ("T", "transparency"),
                    ("c", "show the ssh command"),
                ],
            },
        ],
    ]
}

pub fn draw_help(frame: &mut Frame, app: &AppService, body: Rect) {
    let t = &app.theme;
    let area = centered(72, 21, body);
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
            "Reads and writes ~/.ssh/config, with a backup on every save.",
            t.muted(),
        )),
        Line::from(""),
    ];

    for [left, right] in sections() {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<COLUMN$}", left.title), t.bold_accent_secondary()),
            Span::styled(right.title, t.bold_accent_secondary()),
        ]));

        for row in 0..left.keys.len().max(right.keys.len()) {
            let mut spans = key_row(left.keys.get(row), t);
            spans.extend(key_row(right.keys.get(row), t));
            lines.push(Line::from(spans));
        }

        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(" Mouse ", t.pill(&t.accent_secondary)),
        Span::styled("  cards, tabs, buttons and the hints below all take a click", t.muted()),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" ? ", t.selected()),
        Span::styled(" or ", t.muted()),
        Span::styled(" Esc ", t.selected()),
        Span::styled(" closes this help", t.muted()),
    ]));

    // INFO: on a short screen the blurb and then the spacing give way, so the
    // keys themselves are the last thing to be cut
    let room = block.inner(area).height as usize;
    if lines.len() > room {
        lines.drain(..2);
    }
    while lines.len() > room {
        match lines.iter().position(|line| line.width() == 0) {
            Some(blank) => drop(lines.remove(blank)),
            None => break,
        }
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// One key and what it does. The key wears a cap so the eye can find it
/// without reading the whole line.
fn key_row<'a>(entry: Option<&(&'a str, &'a str)>, t: &crate::models::Theme) -> Vec<Span<'a>> {
    let Some((key, what)) = entry else {
        return vec![Span::raw(" ".repeat(COLUMN))];
    };

    let cap = format!(" {} ", key);
    let pad = CAP.saturating_sub(cap.chars().count());

    vec![
        Span::styled(cap, t.selected()),
        Span::raw(" ".repeat(pad)),
        Span::styled(
            format!("{:<width$}", what, width = COLUMN.saturating_sub(CAP)),
            t.base(),
        ),
    ]
}
