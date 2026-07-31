use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};

/// A live ssh session: the process on one end of a pseudo terminal, and the
/// screen it has painted on the other.
pub struct Session {
    pub alias: String,
    pub screen: Arc<Mutex<vt100::Parser>>,
    running: Arc<AtomicBool>,
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

        let feed = Arc::clone(&screen);
        let alive = Arc::clone(&running);
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
            alive.store(false, Ordering::Relaxed);
        });

        // INFO: nobody waits on the child anywhere else, so it is reaped here
        // and the flag it clears is what marks the tab as finished
        let alive = Arc::clone(&running);
        std::thread::spawn(move || {
            let _ = child.wait();
            alive.store(false, Ordering::Relaxed);
        });

        Ok(Self {
            alias: alias.to_string(),
            screen,
            running,
            writer,
            master: pair.master,
            rows: size.rows,
            columns: size.cols,
        })
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
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

        let deadline = Instant::now() + Duration::from_secs(5);
        while session.is_running() {
            assert!(Instant::now() < deadline, "the session never finished");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
