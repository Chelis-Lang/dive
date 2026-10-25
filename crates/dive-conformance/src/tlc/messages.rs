//! Parser for TLC's `-tool` message stream.
//!
//! With `-tool`, TLC wraps each message as
//!
//! ```text
//! @!@!@STARTMSG <code>:<class> @!@!@
//! <body>
//! @!@!@ENDMSG <code> @!@!@
//! ```
//!
//! (`tlc2.output.MP`). Output that is not a message (SANY's progress and
//! error text, `Print` output, the `-dumpTrace` notice) appears between
//! messages and is kept as untagged text, because SANY's parse and semantic
//! errors are only reported there. Anything malformed is recorded as an
//! anomaly instead of being repaired.

use serde::{Deserialize, Serialize};

const START: &str = "@!@!@STARTMSG ";
const END: &str = "@!@!@ENDMSG ";
const DELIM: &str = " @!@!@";

/// Message severity (`MP.NONE` .. `MP.STATE`).
pub mod class {
    /// Standard output.
    pub const NONE: i32 = 0;
    /// Error.
    pub const ERROR: i32 = 1;
    /// TLC bug.
    pub const TLCBUG: i32 = 2;
    /// Warning.
    pub const WARNING: i32 = 3;
    /// State print.
    pub const STATE: i32 = 4;
}

/// One `-tool` message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// `EC` code.
    pub code: i32,
    /// `MP` class.
    pub class: i32,
    /// Body text without the trailing newline before `ENDMSG`.
    pub body: String,
}

/// A piece of TLC's stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// A delimited message.
    Message(Message),
    /// Text outside any message.
    Text(String),
}

/// A parsed transcript.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transcript {
    /// Messages and untagged text, in output order.
    pub segments: Vec<Segment>,
    /// Malformations found while parsing (unterminated or mismatched
    /// messages, bad headers). Non-empty means the stream cannot be trusted.
    pub anomalies: Vec<String>,
}

impl Transcript {
    /// The messages, in order.
    pub fn messages(&self) -> impl Iterator<Item = &Message> {
        self.segments.iter().filter_map(|s| match s {
            Segment::Message(m) => Some(m),
            Segment::Text(_) => None,
        })
    }

    /// All untagged text, concatenated in order.
    #[must_use]
    pub fn untagged_text(&self) -> String {
        self.segments
            .iter()
            .filter_map(|s| match s {
                Segment::Text(t) => Some(t.as_str()),
                Segment::Message(_) => None,
            })
            .collect()
    }

    /// The first message with `code`.
    #[must_use]
    pub fn first(&self, code: i32) -> Option<&Message> {
        self.messages().find(|m| m.code == code)
    }

    /// The last message with `code`.
    #[must_use]
    pub fn last(&self, code: i32) -> Option<&Message> {
        self.messages().filter(|m| m.code == code).last()
    }
}

fn parse_header(header: &str) -> Option<(i32, i32)> {
    let (code, class) = header.split_once(':')?;
    Some((code.trim().parse().ok()?, class.trim().parse().ok()?))
}

/// The next marker-like text in the output.
enum Candidate<'a> {
    /// A real marker: `header` is the text between the marker and its
    /// closing delimiter; `next` is where output resumes after it.
    Marker {
        at: usize,
        is_start: bool,
        header: &'a str,
        next: usize,
    },
    /// Marker text that TLC did not print as a marker (for example inside a
    /// quoted `Print` string); it is ordinary text.
    NotAMarker { resume: usize },
    /// A line-start `STARTMSG` without a closing delimiter.
    Malformed { what: String },
}

/// Finds the next marker at or after `from`.
///
/// TLC (`MP.java`) prints a `STARTMSG` header followed by a newline (at the
/// start of a line, except when v1.7.4 nests a message after `": "`), and an
/// `ENDMSG` always at the start of a line followed by a newline. Marker text
/// anywhere else, such as `PrintT("@!@!@ENDMSG 2185 @!@!@")`, which TLC
/// prints quoted and mid-line, is not a marker. That way spec output cannot
/// forge or close messages.
fn next_marker(s: &str, from: usize) -> Option<Candidate<'_>> {
    let rest = &s[from..];
    let start = rest.find(START).map(|i| (i + from, true));
    let end = rest.find(END).map(|i| (i + from, false));
    let (at, is_start) = match (start, end) {
        (None, None) => return None,
        (Some(a), None) | (None, Some(a)) => a,
        (Some(a), Some(b)) => {
            if a.0 < b.0 {
                a
            } else {
                b
            }
        }
    };
    let marker = if is_start { START } else { END };
    let line_start = at == 0 || s.as_bytes().get(at - 1) == Some(&b'\n');
    let after_idx = at + marker.len();
    let after = &s[after_idx..];
    let Some(hdr_end) = after.find(DELIM).filter(|e| !after[..*e].contains('\n')) else {
        return Some(if line_start && is_start {
            Candidate::Malformed {
                what: "STARTMSG header without closing delimiter".to_owned(),
            }
        } else {
            Candidate::NotAMarker { resume: after_idx }
        });
    };
    let delim_end = after_idx + hdr_end + DELIM.len();
    let tail = &s[delim_end..];
    let ends_line = tail.is_empty() || tail.starts_with('\n');
    if !ends_line || !(is_start || line_start) {
        return Some(Candidate::NotAMarker { resume: after_idx });
    }
    Some(Candidate::Marker {
        at,
        is_start,
        header: &after[..hdr_end],
        next: if tail.is_empty() {
            delim_end
        } else {
            delim_end + 1
        },
    })
}

/// A message that has started but not ended.
struct Open {
    slot: usize,
    frame: usize,
    code: i32,
    class: i32,
    body: String,
}

/// Parser-owned framing, separate from the publicly mutable transcript.
pub(crate) struct OwnedTranscript {
    transcript: Transcript,
    frames: Vec<Frame>,
}

/// One actual scanner frame; indices refer to this carrier only.
pub(super) struct Frame {
    pub(super) segment: usize,
    pub(super) parent: Option<usize>,
    pub(super) start: usize,
    pub(super) end: Option<usize>,
    pub(super) line_start: bool,
    pub(super) canonical: bool,
    pub(super) parent_body_prefix_len: usize,
    pub(super) own_body_len: usize,
}

impl OwnedTranscript {
    pub(crate) fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub(super) fn frames(&self) -> &[Frame] {
        &self.frames
    }

    pub(super) fn message(&self, frame: usize) -> Option<&Message> {
        match self
            .transcript
            .segments
            .get(self.frames.get(frame)?.segment)?
        {
            Segment::Message(message) => Some(message),
            Segment::Text(_) => None,
        }
    }

    pub(super) fn unique(&self, code: i32) -> Option<usize> {
        let mut found = (0..self.frames.len())
            .filter(|&index| self.message(index).is_some_and(|m| m.code == code));
        let index = found.next()?;
        found.next().is_none().then_some(index)
    }
}

/// Parses TLC's stdout as produced with `-tool`.
///
/// Messages may nest: v1.7.4 prints a complete message inside the body of
/// `GENERAL` (1000) when an exception wraps a reported error (for example a
/// configuration error, `CFG_EXPECTED_SYMBOL`, inside "TLC threw an
/// unexpected exception"). Properly nested messages are parsed; each one
/// becomes its own [`Message`], listed in the order the messages *start*,
/// and the outer body keeps only its own text. An `ENDMSG` that does not
/// close the innermost open message, or a message left open at the end of
/// the output, is an anomaly.
#[must_use]
pub fn parse(stdout: &str) -> Transcript {
    parse_owned(stdout).transcript
}

/// Parses once while retaining immutable scanner ownership for native callers.
#[allow(
    clippy::too_many_lines,
    reason = "a single-pass scanner reads best as one loop"
)]
pub(crate) fn parse_owned(stdout: &str) -> OwnedTranscript {
    let mut slots: Vec<Option<Segment>> = Vec::new();
    let mut anomalies = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut frames = Vec::new();
    let emit_text = |text: &str, stack: &mut Vec<Open>, slots: &mut Vec<Option<Segment>>| {
        if text.is_empty() {
            return;
        }
        match stack.last_mut() {
            Some(open) => open.body.push_str(text),
            None => slots.push(Some(Segment::Text(text.to_owned()))),
        }
    };
    // `pos` is where unconsumed output starts; `scan` is where the search for
    // the next marker resumes (past marker-like text that is not a marker).
    let mut pos = 0;
    let mut scan = 0;
    while let Some(found) = next_marker(stdout, scan) {
        let (at, is_start, header, next) = match found {
            Candidate::Marker {
                at,
                is_start,
                header,
                next,
            } => (at, is_start, header, next),
            Candidate::NotAMarker { resume } => {
                scan = resume;
                continue;
            }
            Candidate::Malformed { what } => {
                anomalies.push(what);
                emit_text(&stdout[pos..], &mut stack, &mut slots);
                pos = stdout.len();
                break;
            }
        };
        emit_text(&stdout[pos..at], &mut stack, &mut slots);
        let marker_end = next;
        if is_start {
            let Some((code, class)) = parse_header(header) else {
                anomalies.push(format!("malformed STARTMSG header `{header}`"));
                emit_text(&stdout[at..], &mut stack, &mut slots);
                pos = stdout.len();
                break;
            };
            let frame = frames.len();
            frames.push(Frame {
                segment: slots.len(),
                parent: stack.last().map(|open| open.frame),
                start: at,
                end: None,
                line_start: at == 0 || stdout.as_bytes().get(at - 1) == Some(&b'\n'),
                canonical: header == format!("{code}:{class}"),
                parent_body_prefix_len: stack.last().map_or(0, |open| open.body.len()),
                own_body_len: 0,
            });
            slots.push(None);
            stack.push(Open {
                slot: slots.len() - 1,
                frame,
                code,
                class,
                body: String::new(),
            });
        } else {
            let code: Option<i32> = header.trim().parse().ok();
            match stack.last() {
                Some(open) if Some(open.code) == code => {
                    if let Some(open) = stack.pop() {
                        frames[open.frame].end = Some(marker_end);
                        frames[open.frame].canonical &= header == open.code.to_string();
                        frames[open.frame].own_body_len = open.body.len();
                        let body = open
                            .body
                            .strip_suffix('\n')
                            .unwrap_or(&open.body)
                            .to_owned();
                        if let Some(slot) = slots.get_mut(open.slot) {
                            *slot = Some(Segment::Message(Message {
                                code: open.code,
                                class: open.class,
                                body,
                            }));
                        }
                    }
                }
                top => {
                    anomalies.push(format!(
                        "ENDMSG `{header}` does not close the innermost open message ({:?})",
                        top.map(|o| o.code)
                    ));
                    emit_text(&stdout[at..marker_end], &mut stack, &mut slots);
                }
            }
        }
        pos = marker_end;
        scan = marker_end;
    }
    emit_text(&stdout[pos..], &mut stack, &mut slots);
    while let Some(open) = stack.pop() {
        anomalies.push(format!(
            "message {} is not terminated by its ENDMSG",
            open.code
        ));
        if let Some(slot) = slots.get_mut(open.slot) {
            *slot = Some(Segment::Message(Message {
                code: open.code,
                class: open.class,
                body: open.body,
            }));
        }
    }
    OwnedTranscript {
        transcript: Transcript {
            segments: slots.into_iter().flatten().collect(),
            anomalies,
        },
        frames,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_messages_and_text() {
        let s = "@!@!@STARTMSG 2262:0 @!@!@\nTLC2 Version x\n@!@!@ENDMSG 2262 @!@!@\nParsing file A.tla\n@!@!@STARTMSG 2217:4 @!@!@\n1: <Initial predicate>\nx = 0\n\n@!@!@ENDMSG 2217 @!@!@\n";
        let t = parse(s);
        assert_eq!(t.anomalies, Vec::<String>::new());
        let msgs: Vec<_> = t.messages().collect();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].code, 2262);
        assert_eq!(msgs[0].body, "TLC2 Version x");
        assert_eq!(msgs[1].class, class::STATE);
        assert_eq!(msgs[1].body, "1: <Initial predicate>\nx = 0\n");
        assert_eq!(t.untagged_text(), "Parsing file A.tla\n");
    }

    #[test]
    fn unterminated_message_is_an_anomaly() {
        let t = parse("@!@!@STARTMSG 2185:0 @!@!@\nStarting...\n");
        assert_eq!(t.anomalies.len(), 1);
        assert_eq!(t.messages().count(), 1);
    }

    #[test]
    fn nested_messages_are_parsed() {
        // As printed by v1.7.4 for a configuration error wrapped in GENERAL.
        let t = parse(
            "@!@!@STARTMSG 1000:1 @!@!@\nTLC threw an unexpected exception.\n: @!@!@STARTMSG 5006:0 @!@!@\nIt was expecting a keyword\n@!@!@ENDMSG 5006 @!@!@\n@!@!@ENDMSG 1000 @!@!@\ntail\n",
        );
        assert_eq!(t.anomalies, Vec::<String>::new());
        let msgs: Vec<_> = t.messages().collect();
        assert_eq!((msgs[0].code, msgs[1].code), (1000, 5006));
        assert_eq!(msgs[0].body, "TLC threw an unexpected exception.\n: ");
        assert_eq!(msgs[1].body, "It was expecting a keyword");
        assert_eq!(t.untagged_text(), "tail\n");
    }

    #[test]
    fn crossed_messages_are_an_anomaly() {
        let t = parse(
            "@!@!@STARTMSG 1:1 @!@!@\na\n@!@!@STARTMSG 2:1 @!@!@\nb\n@!@!@ENDMSG 1 @!@!@\n@!@!@ENDMSG 2 @!@!@\n",
        );
        assert_ne!(t.anomalies, Vec::<String>::new());
        let t = parse("x\n@!@!@ENDMSG 7 @!@!@\n");
        assert_ne!(t.anomalies, Vec::<String>::new());
    }

    #[test]
    fn bad_header_is_an_anomaly() {
        let t = parse("@!@!@STARTMSG x:y @!@!@\n");
        assert_eq!(t.anomalies.len(), 1);
    }
}
