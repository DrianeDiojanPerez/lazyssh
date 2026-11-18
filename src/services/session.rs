use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};

use crossterm::event::{KeyModifiers, MouseEventKind};

use super::mouse;
use super::replies::{Found, Questions};

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
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    copies: Arc<Mutex<Vec<Vec<u8>>>>,
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
        let writer = Arc::new(Mutex::new(
            pair.master
                .take_writer()
                .map_err(|e| format!("cannot write to the session: {}", e))?,
        ));

        let screen = Arc::new(Mutex::new(vt100::Parser::new(size.rows, size.cols, 2000)));
        let running = Arc::new(AtomicBool::new(true));
        let spoken = Arc::new(AtomicBool::new(false));
        let exit = Arc::new(AtomicI32::new(UNFINISHED));

        // INFO: the reader stops at the end of the output but says nothing
        // about the process: the terminal closing is not the same as ssh
        // being gone, and only the one below knows that
        let feed = Arc::clone(&screen);
        let heard = Arc::clone(&spoken);
        let reply = Arc::clone(&writer);
        let copies = Arc::new(Mutex::new(Vec::new()));
        let copied = Arc::clone(&copies);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            let mut questions = Questions::default();
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        let output = &buffer[..read];
                        let mut answers = Vec::new();
                        if let Ok(mut screen) = feed.lock() {
                            // INFO: everything up to a question is drawn before
                            // it is answered, so the cursor is where the
                            // program expects it to be
                            let mut drawn = 0;
                            for (end, found) in questions.find(output) {
                                screen.process(&output[drawn..end]);
                                drawn = end;
                                match found {
                                    Found::Question(question) => {
                                        answers.extend(question.answer(screen.screen().cursor_position()))
                                    }
                                    Found::Copy(sequence) => {
                                        if let Ok(mut copies) = copied.lock() {
                                            copies.push(sequence);
                                        }
                                    }
                                }
                            }
                            screen.process(&output[drawn..]);
                        }
                        if !answers.is_empty() {
                            if let Ok(mut writer) = reply.lock() {
                                let _ = writer.write_all(&answers);
                                let _ = writer.flush();
                            }
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
            copies,
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

    /// What the remote end asked to put on the clipboard since the last call,
    /// each one a whole OSC 52 sequence for the real terminal to carry out.
    pub fn take_copies(&self) -> Vec<Vec<u8>> {
        self.copies.lock().map(|mut copies| std::mem::take(&mut *copies)).unwrap_or_default()
    }

    /// Pastes `text` the way the remote program asked for it: marked as a paste
    /// when it turned that on, so an editor does not indent it as typing.
    pub fn paste(&mut self, text: &str) {
        let marked = self.screen.lock().map(|screen| screen.screen().bracketed_paste()).unwrap_or(false);
        let bytes = match marked {
            true => [b"\x1b[200~".as_slice(), text.as_bytes(), b"\x1b[201~"].concat(),
            false => text.as_bytes().to_vec(),
        };
        self.send(&bytes);
    }

    /// Reports a mouse event to the remote program if it turned the mouse on.
    /// `column` and `row` count from zero inside the session. Whether anything
    /// was sent, so the caller knows if lazyssh should handle it instead.
    pub fn mouse(&mut self, kind: MouseEventKind, modifiers: KeyModifiers, column: u16, row: u16) -> bool {
        let Ok((mode, encoding)) = self
            .screen
            .lock()
            .map(|screen| (screen.screen().mouse_protocol_mode(), screen.screen().mouse_protocol_encoding()))
        else {
            return false;
        };

        match mouse::encode(mode, encoding, kind, modifiers, column, row) {
            Some(bytes) => {
                self.send(&bytes);
                true
            }
            None => false,
        }
    }

    pub fn send(&mut self, bytes: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
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
    fn a_program_that_asks_what_terminal_this_is_gets_an_answer() {
        let script = r#"stty -echo; printf '\033[c'; if IFS= read -r -t 2 -d c reply; then echo answered; else echo silent; fi"#;
        let session = Session::spawn("test", "bash", &["-c".to_string(), script.to_string()], 10, 40)
            .expect("the pty should start");

        wait_for_the_end(&session);
        let printed = session.screen.lock().unwrap().screen().contents();
        assert!(printed.contains("answered"), "the question went unanswered:\n{}", printed);
    }

    #[test]
    fn a_program_that_asks_where_the_cursor_is_gets_the_spot() {
        let script = r#"stty -echo; printf 'abc\033[6n'; IFS= read -r -t 2 -d R reply; echo; echo "at ${reply#*[}""#;
        let session = Session::spawn("test", "bash", &["-c".to_string(), script.to_string()], 10, 40)
            .expect("the pty should start");

        wait_for_the_end(&session);
        let printed = session.screen.lock().unwrap().screen().contents();
        assert!(printed.contains("at 1;4"), "the cursor was not reported:\n{}", printed);
    }

    #[test]
    fn a_copy_from_the_remote_end_is_handed_over_once() {
        let script = r#"printf 'before\033]52;c;aGVsbG8=\007after'"#;
        let session = Session::spawn("test", "bash", &["-c".to_string(), script.to_string()], 10, 40)
            .expect("the pty should start");

        wait_for_the_end(&session);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut copies = session.take_copies();
        while copies.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            copies = session.take_copies();
        }

        assert_eq!(copies, vec![b"\x1b]52;c;aGVsbG8=\x07".to_vec()]);
        assert!(session.take_copies().is_empty());
    }

    #[test]
    fn a_click_reaches_a_program_that_turned_the_mouse_on() {
        let script = r#"stty -echo; printf '\033[?1000h\033[?1006hready'; IFS= read -r -t 2 -d M click; printf '\033[?1000l\033[?1006l\n'; printf 'got %q\n' "$click""#;
        let mut session = Session::spawn("test", "bash", &["-c".to_string(), script.to_string()], 10, 60)
            .expect("the pty should start");
        wait_until_ready(&session);

        let sent = session.mouse(MouseEventKind::Down(crossterm::event::MouseButton::Left), KeyModifiers::NONE, 4, 2);

        wait_for_the_end(&session);
        let printed = session.screen.lock().unwrap().screen().contents();
        assert!(sent, "the click was not sent");
        assert!(printed.contains(r"got $'\E[<0;5;3'"), "the click did not arrive:\n{}", printed);
    }

    #[test]
    fn a_click_is_left_to_lazyssh_when_the_program_did_not_ask() {
        let mut session = Session::spawn("test", "bash", &["-c".to_string(), "printf ready; sleep 2".to_string()], 10, 60)
            .expect("the pty should start");
        wait_until_ready(&session);

        assert!(!session.mouse(MouseEventKind::ScrollUp, KeyModifiers::NONE, 4, 2));
    }

    fn wait_until_ready(session: &Session) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !session.screen.lock().unwrap().screen().contents().contains("ready") {
            assert!(Instant::now() < deadline, "the program never got ready");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_paste_is_marked_when_the_program_asked_for_it() {
        let script = r#"stty -echo; printf '\033[?2004hready'; IFS= read -r -t 2 -d '~' a; IFS= read -r -t 2 -d '~' b; printf '\033[?2004l\n'; printf 'got %q\n' "${a}~${b}~""#;

        let printed = pasted_into(script, "hi");

        assert!(printed.contains(r"got $'\E[200~hi\E[201~'"), "the paste was not marked:\n{}", printed);
    }

    #[test]
    fn a_paste_is_plain_text_when_the_program_did_not_ask() {
        let script = r#"stty -echo; printf 'ready'; IFS= read -r -t 2 -n 2 a; echo; printf 'got %q\n' "$a""#;

        let printed = pasted_into(script, "hi");

        assert!(printed.contains("got hi"), "the paste did not arrive as typed:\n{}", printed);
    }

    /// Runs `script`, pastes `text` once it has printed "ready", and returns
    /// what was on the screen when it ended.
    fn pasted_into(script: &str, text: &str) -> String {
        let mut session = Session::spawn("test", "bash", &["-c".to_string(), script.to_string()], 10, 60)
            .expect("the pty should start");

        let deadline = Instant::now() + Duration::from_secs(5);
        while !session.screen.lock().unwrap().screen().contents().contains("ready") {
            assert!(Instant::now() < deadline, "the program never got ready");
            std::thread::sleep(Duration::from_millis(20));
        }
        session.paste(text);

        wait_for_the_end(&session);
        let printed = session.screen.lock().unwrap().screen().contents();
        printed
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
