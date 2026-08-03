use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};

/// What the exit holds while the process is still there. No real exit code
/// looks like this, so it doubles as "it has not said yet".
const UNFINISHED: i32 = i32::MIN;

/// How long a connection is shown as being made even after the far end has
/// answered. A host on the same network answers before the eye can follow it,
/// and a screen that flickers past reads as a fault rather than as a
/// connection being made.
pub const SETTLE: Duration = Duration::from_secs(2);

pub struct Session {
    pub alias: String,
    pub screen: Arc<Mutex<vt100::Parser>>,
    running: Arc<AtomicBool>,
    spoken: Arc<AtomicBool>,
    opened: Instant,
    exit: Arc<AtomicI32>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    rows: u16,
    columns: u16,
}

impl Session {
    pub fn open(alias: &str, args: &[String], rows: u16, columns: u16) -> Result<Self, String> {
        Self::spawn(alias, "ssh", args, rows, columns)
    }

    pub fn spawn(
        alias: &str,
        program: &str,
        args: &[String],
        rows: u16,
        columns: u16,
    ) -> Result<Self, String> {
        let size = PtySize {
            rows: rows.max(1),
            cols: columns.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = NativePtySystem::default()
            .openpty(size)
            .map_err(|e| format!("cannot open a terminal: {}", e))?;

        let mut command = CommandBuilder::new(program);
        command.args(args);
        command.env("TERM", "xterm-256color");

        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|e| format!("cannot start {}: {}", program, e))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("cannot read the session: {}", e))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("cannot write to the session: {}", e))?;

        let screen = Arc::new(Mutex::new(vt100::Parser::new(size.rows, size.cols, 2000)));
        let running = Arc::new(AtomicBool::new(true));
        let spoken = Arc::new(AtomicBool::new(false));
        let exit = Arc::new(AtomicI32::new(UNFINISHED));

        // INFO: the reader stops at the end of the output but says nothing
        // about the process: the terminal closing is not the same as ssh
        // being gone, and only the one below knows that
        let feed = Arc::clone(&screen);
        let heard = Arc::clone(&spoken);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if let Ok(mut screen) = feed.lock() {
                            screen.process(&buffer[..read]);
                        }
                        heard.store(true, Ordering::SeqCst);
                    }
                }
            }
        });

        // INFO: nobody waits on the child anywhere else, so it is reaped here.
        // The code is put down before the flag is cleared, so whoever sees the
        // session stop can trust what it says about how it ended
        let alive = Arc::clone(&running);
        let code = Arc::clone(&exit);
        std::thread::spawn(move || {
            let status = child.wait().map(|status| status.exit_code() as i32);
            code.store(status.unwrap_or(-1), Ordering::SeqCst);
            alive.store(false, Ordering::SeqCst);
        });

        Ok(Self {
            alias: alias.to_string(),
            screen,
            running,
            spoken,
            opened: Instant::now(),
            exit,
            writer,
            master: pair.master,
            rows: size.rows,
            columns: size.cols,
        })
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Whether the far end has sent anything at all. Until it has there is
    /// nothing to paint, and ssh is still off resolving, connecting or agreeing
    /// on keys.
    pub fn has_spoken(&self) -> bool {
        self.spoken.load(Ordering::SeqCst)
    }

    pub fn waiting_for(&self) -> Duration {
        self.opened.elapsed()
    }

    /// Whether the session is still being made rather than being used, which is
    /// what is worth saying so out loud for.
    pub fn is_connecting(&self) -> bool {
        self.is_running() && (!self.has_spoken() || self.waiting_for() < SETTLE)
    }

    pub fn exit_code(&self) -> Option<i32> {
        match self.exit.load(Ordering::SeqCst) {
            UNFINISHED => None,
            code => Some(code),
        }
    }

    /// Whether ssh itself gave up, rather than the session having run and
    /// ended. ssh keeps 255 for its own troubles and hands back whatever the
    /// remote shell exited with otherwise: 0 for a logout, 130 for a Ctrl-C at
    /// the prompt, and every one of those was a session that really ran.
    pub fn ssh_failed(&self) -> bool {
        matches!(self.exit_code(), Some(255) | Some(-1))
    }

    pub fn is_finished(&self) -> bool {
        !self.is_running() && !self.ssh_failed()
    }

    pub fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&mut self, rows: u16, columns: u16) {
        let (rows, columns) = (rows.max(1), columns.max(1));
        if rows == self.rows && columns == self.columns {
            return;
        }

        self.rows = rows;
        self.columns = columns;

        let _ = self.master.resize(PtySize {
            rows,
            cols: columns,
            pixel_width: 0,
            pixel_height: 0,
        });

        if let Ok(mut screen) = self.screen.lock() {
            screen.screen_mut().set_size(rows, columns);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_session_shows_what_the_process_prints() {
        let session = Session::spawn("test", "echo", &["hello".to_string()], 10, 40)
            .expect("the pty should have started");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let printed = session.screen.lock().unwrap().screen().contents();
            if printed.contains("hello") {
                break;
            }

            assert!(Instant::now() < deadline, "nothing was ever read:\n{}", printed);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_session_that_ends_stops_reporting_itself_as_running() {
        let session = Session::spawn("test", "true", &[], 10, 40).expect("the pty should start");

        wait_for_the_end(&session);
    }

    #[test]
    fn a_session_that_logged_out_is_finished() {
        let session = Session::spawn("test", "true", &[], 10, 40).expect("the pty should start");

        wait_for_the_end(&session);

        assert_eq!(session.exit_code(), Some(0));
        assert!(session.is_finished());
    }

    #[test]
    fn a_prompt_left_on_a_ctrl_c_is_finished_all_the_same() {
        let session = ending_with(130);

        assert_eq!(session.exit_code(), Some(130));
        assert!(session.is_finished());
    }

    #[test]
    fn ssh_giving_up_is_not_a_finished_session() {
        let session = ending_with(255);

        assert!(session.ssh_failed());
        assert!(!session.is_finished());
    }

    fn ending_with(code: i32) -> Session {
        let args = ["-c".to_string(), format!("exit {}", code)];
        let session = Session::spawn("test", "sh", &args, 10, 40).expect("the pty should start");

        wait_for_the_end(&session);
        session
    }

    /// The flag is only cleared once the process has been waited on, so a
    /// session that has stopped always has its code to hand.
    fn wait_for_the_end(session: &Session) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while session.is_running() {
            assert!(Instant::now() < deadline, "the session never finished");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
