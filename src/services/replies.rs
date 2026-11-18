//! Programs ask the terminal questions by printing them, and a real terminal
//! answers by typing the reply back in. vt100 only draws, so without this a
//! program that waits for an answer sits out its whole timeout: nvim gives
//! every start and every quit a full second.
//!
//! Programs also print what they want put on the clipboard (OSC 52). vt100
//! drops that too, so it is picked out here to be handed to the real terminal.

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

/// The longest question kept from the end of one read to finish it in the next.
const LONGEST: usize = 64;

/// The longest copy kept waiting for its end. A copy is the text itself in
/// base64, so this is room for roughly 750 KB of yanked text.
const LONGEST_COPY: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// `CSI c`: what kind of terminal this is. Many programs ask it last, so
    /// its answer tells them every other reply that was coming has arrived.
    Attributes,
    /// `CSI 6 n`: where the cursor is.
    CursorPosition,
}

impl Question {
    /// What to type back. `cursor` is the zero-based row and column right
    /// after the question was printed.
    pub fn answer(&self, cursor: (u16, u16)) -> Vec<u8> {
        match self {
            // INFO: the 52 says copying to the clipboard works, which is what
            // nvim looks for before it will copy over ssh
            Question::Attributes => b"\x1b[?62;22;52c".to_vec(),
            Question::CursorPosition => format!("\x1b[{};{}R", cursor.0 + 1, cursor.1 + 1).into_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    Question(Question),
    /// A whole OSC 52 sequence asking to set the clipboard, ready to pass on.
    Copy(Vec<u8>),
}

#[derive(Default)]
pub struct Questions {
    carried: Vec<u8>,
}

impl Questions {
    /// The questions and copies in `chunk`, each with the position in `chunk`
    /// just past its last byte. One cut off at the end is kept, and found once
    /// the next chunk finishes it.
    pub fn find(&mut self, chunk: &[u8]) -> Vec<(usize, Found)> {
        let carried = self.carried.len();
        let mut bytes = std::mem::take(&mut self.carried);
        bytes.extend_from_slice(chunk);

        let mut found = Vec::new();
        let mut at = 0;
        while let Some(offset) = bytes[at..].iter().position(|&b| b == ESC) {
            let start = at + offset;
            match read_sequence(&bytes[start..]) {
                Sequence::Unfinished(longest) => {
                    if bytes.len() - start <= longest {
                        self.carried = bytes[start..].to_vec();
                    }
                    break;
                }
                Sequence::Done(length, item) => {
                    let end = start + length;
                    if let Some(item) = item {
                        found.push((end - carried, item));
                    }
                    at = end;
                }
            }
        }

        found
    }
}

enum Sequence {
    /// Not ended yet, and worth keeping for the next read up to this length.
    Unfinished(usize),
    Done(usize, Option<Found>),
}

fn read_sequence(bytes: &[u8]) -> Sequence {
    match bytes.get(1) {
        None => Sequence::Unfinished(LONGEST),
        Some(b'[') => read_csi(bytes),
        Some(b']') => read_osc(bytes),
        Some(_) => Sequence::Done(1, None),
    }
}

fn read_osc(bytes: &[u8]) -> Sequence {
    let mut at = 2;
    while at < bytes.len() {
        match bytes[at] {
            BEL => return osc(bytes, at, at + 1),
            ESC => match bytes.get(at + 1) {
                None => break,
                Some(b'\\') => return osc(bytes, at, at + 2),
                // INFO: a new sequence starting means this one was cut short
                Some(_) => return Sequence::Done(at, None),
            },
            _ => at += 1,
        }
    }

    Sequence::Unfinished(LONGEST_COPY)
}

/// `body_end` is where the terminator starts and `end` is just past it.
fn osc(bytes: &[u8], body_end: usize, end: usize) -> Sequence {
    let body = &bytes[2..body_end];
    let Some(rest) = body.strip_prefix(b"52;") else {
        return Sequence::Done(end, None);
    };

    // INFO: a `?` asks to read the clipboard rather than set it, and the
    // answer would have to come back through lazyssh's own input
    let data = rest.splitn(2, |&b| b == b';').nth(1).unwrap_or_default();
    match data {
        b"?" => Sequence::Done(end, None),
        _ => Sequence::Done(end, Some(Found::Copy(bytes[..end].to_vec()))),
    }
}

fn read_csi(bytes: &[u8]) -> Sequence {
    let Some(offset) = bytes[2..].iter().position(|b| (0x40..=0x7e).contains(b)) else {
        return Sequence::Unfinished(LONGEST);
    };

    let end = 2 + offset;
    let question = match (&bytes[2..end], bytes[end]) {
        (b"" | b"0", b'c') => Some(Question::Attributes),
        (b"6", b'n') => Some(Question::CursorPosition),
        _ => None,
    };

    Sequence::Done(end + 1, question.map(Found::Question))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_terminal_kind_question_is_found_where_it_ends() {
        let found = Questions::default().find(b"hi\x1b[c there");

        assert_eq!(found, vec![(5, Found::Question(Question::Attributes))]);
    }

    #[test]
    fn the_terminal_kind_question_can_spell_out_its_zero() {
        let found = Questions::default().find(b"\x1b[0c");

        assert_eq!(found, vec![(4, Found::Question(Question::Attributes))]);
    }

    #[test]
    fn the_cursor_question_is_found() {
        let found = Questions::default().find(b"\x1b[6n");

        assert_eq!(found, vec![(4, Found::Question(Question::CursorPosition))]);
    }

    #[test]
    fn colors_and_moves_are_not_questions() {
        let found = Questions::default().find(b"\x1b[1;31mred\x1b[0m\x1b[2J\x1b[?2026$p\x1b[>0c");

        assert!(found.is_empty());
    }

    #[test]
    fn a_question_cut_between_two_reads_is_found_in_the_second() {
        let mut questions = Questions::default();

        assert!(questions.find(b"text\x1b[").is_empty());
        assert_eq!(questions.find(b"c more"), vec![(1, Found::Question(Question::Attributes))]);
    }

    #[test]
    fn a_lone_escape_at_the_end_waits_for_the_rest() {
        let mut questions = Questions::default();

        assert!(questions.find(b"text\x1b").is_empty());
        assert_eq!(questions.find(b"[6n"), vec![(3, Found::Question(Question::CursorPosition))]);
    }

    #[test]
    fn the_cursor_answer_counts_from_one() {
        assert_eq!(Question::CursorPosition.answer((0, 4)), b"\x1b[1;5R".to_vec());
    }

    #[test]
    fn the_terminal_kind_answer_says_copying_works() {
        assert_eq!(Question::Attributes.answer((0, 0)), b"\x1b[?62;22;52c".to_vec());
    }

    #[test]
    fn a_copy_is_picked_out_whole_with_either_ending() {
        let bell = b"\x1b]52;c;aGVsbG8=\x07".to_vec();
        let string_end = b"\x1b]52;c;aGVsbG8=\x1b\\".to_vec();

        assert_eq!(Questions::default().find(&bell), vec![(bell.len(), Found::Copy(bell.clone()))]);
        assert_eq!(
            Questions::default().find(&string_end),
            vec![(string_end.len(), Found::Copy(string_end.clone()))]
        );
    }

    #[test]
    fn a_copy_cut_between_reads_is_picked_out_whole() {
        let mut questions = Questions::default();

        assert!(questions.find(b"text\x1b]52;c;aGVs").is_empty());
        assert_eq!(
            questions.find(b"bG8=\x07 more"),
            vec![(5, Found::Copy(b"\x1b]52;c;aGVsbG8=\x07".to_vec()))]
        );
    }

    #[test]
    fn reading_the_clipboard_and_titles_are_left_alone() {
        let found = Questions::default().find(b"\x1b]52;c;?\x07\x1b]0;my title\x07\x1b]11;?\x1b\\");

        assert!(found.is_empty());
    }

    #[test]
    fn a_question_after_a_copy_is_still_found() {
        let found = Questions::default().find(b"\x1b]52;c;eA==\x07\x1b[c");

        assert_eq!(
            found,
            vec![
                (12, Found::Copy(b"\x1b]52;c;eA==\x07".to_vec())),
                (15, Found::Question(Question::Attributes)),
            ]
        );
    }
}
