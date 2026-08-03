mod input;
mod models;
mod repositories;
mod services;
#[cfg(test)]
mod test_support;
mod ui;

use std::io;
use std::process::Command;
use std::time::Instant;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use models::Action;
use repositories::{FileSshRepository, FileThemeRepository};
use services::AppService;

/// The installer that put lazyssh here. Updating is that same script run
/// again: it already knows the platform, what the releases are called and
/// where the binary belongs, so none of that is worth keeping twice.
#[cfg(unix)]
const INSTALLER: &str =
    "https://raw.githubusercontent.com/DrianeDiojanPerez/lazyssh/refs/heads/master/install.sh";

pub fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if asked_for(&args, "--version", "-V") {
        println!("v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if asked_for(&args, "--update", "-u") {
        return update();
    }

    if asked_for(&args, "--help", "-h") {
        println!("lazyssh {}", env!("CARGO_PKG_VERSION"));
        println!("A TUI SSH manager that reads/edits ~/.ssh/config directly\n");
        println!("Usage: lazyssh [OPTIONS]\n");
        println!("Options:");
        println!("  -V, --version    Print version");
        println!("  -u, --update     Update to the latest release");
        println!("  -h, --help       Print this help");
        return Ok(());
    }

    let ssh_repo = FileSshRepository::new();
    let theme_repo = FileThemeRepository::new();

    let mut app = AppService::initialize(&ssh_repo, &theme_repo);

    loop {
        match run_tui(&mut app, &ssh_repo, &theme_repo)? {
            Action::LaunchSsh(args) => {
                run_in_the_whole_terminal(args);
                app.reload_from_disk(&ssh_repo);
            }
            _ => break,
        }
    }

    Ok(())
}

fn asked_for(args: &[String], long: &str, short: &str) -> bool {
    args.iter().any(|arg| arg == long || arg == short)
}

/// Fetches the installer and runs it, which is all an update is. It is put in
/// a file rather than piped into a shell so that it keeps the terminal: the
/// sudo prompt it may need still has somewhere to ask.
#[cfg(unix)]
fn update() -> io::Result<()> {
    println!("lazyssh v{}, looking for a newer one\n", env!("CARGO_PKG_VERSION"));

    let script = std::env::temp_dir().join("lazyssh-install.sh");

    let fetched = Command::new("curl")
        .args(["-fsSL", INSTALLER, "-o"])
        .arg(&script)
        .status();

    match fetched {
        Ok(status) if status.success() => {}
        Ok(_) => give_up("the installer could not be downloaded"),
        Err(e) => give_up(&format!("curl is needed to update: {}", e)),
    }

    let ran = Command::new("bash").arg(&script).status();
    let _ = std::fs::remove_file(&script);

    match ran {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => give_up("the installer did not finish"),
        Err(e) => give_up(&format!("bash is needed to update: {}", e)),
    }
}

#[cfg(not(unix))]
fn update() -> io::Result<()> {
    println!("lazyssh v{}\n", env!("CARGO_PKG_VERSION"));
    println!("Updating in place is not supported here yet. The latest build is at");
    println!("https://github.com/DrianeDiojanPerez/lazyssh/releases/latest");
    Ok(())
}

#[cfg(unix)]
fn give_up(reason: &str) -> ! {
    eprintln!("\nUpdate failed: {}", reason);
    eprintln!("The latest build is at https://github.com/DrianeDiojanPerez/lazyssh/releases/latest");
    std::process::exit(1);
}

fn run_in_the_whole_terminal(args: Vec<String>) {
    let display = args.join(" ");
    println!("\x1b[1;36m══ ssh {} ══\x1b[0m\n", display);

    let status = Command::new("ssh")
        .args([
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
        Ok(exit) => println!(
            "\n\x1b[1;33m═══ session ended (exit: {}) ═══\x1b[0m",
            exit.code().unwrap_or(-1)
        ),
        Err(e) => println!("\n\x1b[1;31m═══ ssh failed: {} ═══\x1b[0m", e),
    }

    println!("\x1b[90mReturning to lazyssh...\x1b[0m\n");
}

fn run_tui(
    app: &mut AppService,
    ssh_repo: &FileSshRepository,
    theme_repo: &FileThemeRepository,
) -> io::Result<Action> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut last_frame = Instant::now();
    let mut clicks = input::Clicks::default();

    let action = loop {
        app.reap_finished_sessions();

        // INFO: the pty is told the size of the pane before it is drawn, so
        // the remote end lays out for the space it actually has
        let area = terminal.size()?;
        let pane = ui::renderer::frames(app, area).main;
        if let Some(session) = app.active_session_mut() {
            session.resize(pane.height.saturating_sub(2), pane.width.saturating_sub(2));
        }

        terminal.draw(|frame| ui::render(frame, app))?;

        // INFO: toasts age by however long the frame took, but only while some
        // were already on screen: with none, the loop sits in a blocking read
        // and that whole wait would otherwise expire the toast it wakes up for
        let animating = app.has_toasts() || app.has_live_session() || app.probes.is_working();
        input::handle_next_event(app, ssh_repo, theme_repo, &mut clicks)?;

        let now = Instant::now();
        if animating {
            app.advance_toasts(now.duration_since(last_frame));
        }
        last_frame = now;

        let action = app.take_action();
        if !matches!(action, Action::Continue) {
            break action;
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

#[cfg(test)]
mod tests {
    use super::asked_for;

    fn args(given: &[&str]) -> Vec<String> {
        given.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn a_flag_is_taken_by_either_of_its_names() {
        assert!(asked_for(&args(&["lazyssh", "--update"]), "--update", "-u"));
        assert!(asked_for(&args(&["lazyssh", "-u"]), "--update", "-u"));
    }

    #[test]
    fn nothing_else_counts_as_that_flag() {
        assert!(!asked_for(&args(&["lazyssh"]), "--update", "-u"));
        assert!(!asked_for(&args(&["lazyssh", "--updates"]), "--update", "-u"));
        assert!(!asked_for(&args(&["lazyssh", "-V"]), "--update", "-u"));
    }
}
