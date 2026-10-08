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

/// A message that has started but not ended.
struct Open {
    slot: usize,
    code: i32,
    class: i32,
    body: String,
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
#[allow(
    clippy::too_many_lines,
    reason = "a single-pass scanner reads best as one loop"
)]
pub fn parse(stdout: &str) -> Transcript {
    let mut slots: Vec<Option<Segment>> = Vec::new();
    let mut anomalies = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut rest = stdout;
    let emit_text = |text: &str, stack: &mut Vec<Open>, slots: &mut Vec<Option<Segment>>| {
        if text.is_empty() {
            return;
        }
        match stack.last_mut() {
            Some(open) => open.body.push_str(text),
            None => slots.push(Some(Segment::Text(text.to_owned()))),
        }
    };
    loop {
        let next_start = rest.find(START);
        let next_end = rest.find(END);
        let (at, is_start) = match (next_start, next_end) {
            (None, None) => break,
            (Some(s), None) => (s, true),
            (None, Some(e)) => (e, false),
            (Some(s), Some(e)) => (s.min(e), s < e),
        };
        emit_text(&rest[..at], &mut stack, &mut slots);
        let marker = if is_start { START } else { END };
        let after = &rest[at + marker.len()..];
        let Some(hdr_end) = after.find(DELIM) else {
            anomalies.push("message marker without closing delimiter".to_owned());
            emit_text(&rest[at..], &mut stack, &mut slots);
            rest = "";
            break;
        };
        let header = &after[..hdr_end];
        let mut next = &after[hdr_end + DELIM.len()..];
        if let Some(stripped) = next.strip_prefix('\n') {
            next = stripped;
        }
        if is_start {
            let Some((code, class)) = parse_header(header) else {
                anomalies.push(format!("malformed STARTMSG header `{header}`"));
                emit_text(&rest[at..], &mut stack, &mut slots);
                rest = "";
                break;
            };
            slots.push(None);
            stack.push(Open {
                slot: slots.len() - 1,
                code,
                class,
                body: String::new(),
            });
        } else {
            let code: Option<i32> = header.trim().parse().ok();
            match stack.last() {
                Some(open) if Some(open.code) == code => {
                    if let Some(open) = stack.pop() {
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
                    emit_text(
                        &rest[at..at + marker.len() + hdr_end + DELIM.len()],
                        &mut stack,
                        &mut slots,
                    );
                }
            }
        }
        rest = next;
    }
    emit_text(rest, &mut stack, &mut slots);
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
    Transcript {
        segments: slots.into_iter().flatten().collect(),
        anomalies,
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
