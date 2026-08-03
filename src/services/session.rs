use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};

/// What the exit holds while the process is still there. No real exit code
/// looks like this, so it doubles as "it has not said yet".
const UNFINISHED: i32 = i32::MIN;

/// A live ssh session: the process on one end of a pseudo terminal, and the
/// screen it has painted on the other.
pub struct Session {
    pub alias: String,
    pub screen: Arc<Mutex<vt100::Parser>>,
    running: Arc<AtomicBool>,
    exit: Arc<AtomicI32>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    rows: u16,
    columns: u16,
}

impl Session {
    /// Starts ssh on a pseudo terminal of the given size. The output is read on
    /// a thread of its own so a busy session never holds up the interface.
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
        let exit = Arc::new(AtomicI32::new(UNFINISHED));

        // INFO: the reader stops at the end of the output but says nothing
        // about the process: the terminal closing is not the same as ssh
        // being gone, and only the one below knows that
        let feed = Arc::clone(&screen);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if let Ok(mut screen) = feed.lock() {
                            screen.process(&buffer[..read]);
                        }
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

    /// How the process ended, once it has.
    pub fn exit_code(&self) -> Option<i32> {
        match self.exit.load(Ordering::SeqCst) {
            UNFINISHED => None,
            code => Some(code),
        }
    }

    /// A session you logged out of, as against one that fell over: ssh leaves
    /// anything it could not do behind on the screen and a code to match.
    pub fn ended_cleanly(&self) -> bool {
        !self.is_running() && self.exit_code() == Some(0)
    }

    pub fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Keeps the session the same size as the pane it is drawn in. Doing
    /// nothing when the size has not changed keeps it off the hot path.
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
    fn a_session_that_logged_out_says_it_ended_cleanly() {
        let session = Session::spawn("test", "true", &[], 10, 40).expect("the pty should start");

        wait_for_the_end(&session);

        assert_eq!(session.exit_code(), Some(0));
        assert!(session.ended_cleanly());
    }

    #[test]
    fn a_session_that_failed_is_not_a_clean_ending() {
        let session = Session::spawn("test", "false", &[], 10, 40).expect("the pty should start");

        wait_for_the_end(&session);

        assert_eq!(session.exit_code(), Some(1));
        assert!(!session.ended_cleanly());
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
