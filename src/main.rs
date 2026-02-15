mod input;
mod models;
mod repositories;
mod services;
mod ui;

use std::io;
use std::process::Command;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use models::Action;
use repositories::{FileSshRepository, FileThemeRepository};
use services::AppService;

pub fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("lazyssh {}", env!("CARGO_PKG_VERSION"));
        println!("A TUI SSH manager that reads/edits ~/.ssh/config directly\n");
        println!("Usage: lazyssh [OPTIONS]\n");
        println!("Options:");
        println!("  -V, --version    Print version");
        println!("  -h, --help       Print this help");
        return Ok(());
    }

    let ssh_repo = FileSshRepository::new();
    let theme_repo = FileThemeRepository::new();

    let mut app = AppService::initialize(&ssh_repo, &theme_repo);

    loop {
        let action = run_tui_until_action(&mut app, &ssh_repo, &theme_repo)?;

        match action {
            Action::Quit => break,
            Action::LaunchSsh(args) => {
                execute_ssh_session(args);
                app.reload_from_disk(&ssh_repo);
            }
            Action::Continue => {}
        }
    }

    Ok(())
}

fn run_tui_until_action(
    app: &mut AppService,
    ssh_repo: &FileSshRepository,
    theme_repo: &FileThemeRepository,
) -> io::Result<Action> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let action = loop {
        terminal.draw(|frame| ui::render(frame, app))?;
        input::handle_next_event(app, ssh_repo, theme_repo)?;

        let action = app.take_action();
        match action {
            Action::Continue => continue,
            _ => break action,
        }
    };

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(action)
}

fn execute_ssh_session(args: Vec<String>) {
    let display = args.join(" ");

    println!("\x1b[1;36m══ ssh {} ══\x1b[0m\n", display);

    let status = Command::new("ssh")
        .args(&[
            "-o",
            "ConnectTimeout=1",
            "-o",
            "ServerAliveInterval=2",
            "-o",
            "ServerAliveCountMax=2",
        ])
        .args(&args)
        .status();

    match status {
        Ok(exit) => {
            println!(
                "\n\x1b[1;33m═══ SSH session ended (exit: {}) ═══\x1b[0m",
                exit.code().unwrap_or(-1)
            );
        }
        Err(e) => {
            println!("\n\x1b[1;31m═══ SSH failed: {} ═══\x1b[0m", e);
        }
    }

    println!("\x1b[90mReturning to SSH Manager...\x1b[0m\n");
}
