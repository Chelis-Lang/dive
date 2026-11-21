//! Documents whose states are produced one at a time.
//!
//! A [`TraceDoc`] holds every state as a JSON object, which for a trace of
//! millions of states costs many times the states themselves. A producer that
//! already holds its states (simulation keeps its own rows) can instead give
//! a [`StateSource`] with a header document whose `states`, `labels` and
//! `witnesses` are empty: [`write_document`] writes the same bytes as
//! [`TraceDoc::to_json`] without building the document, and
//! [`crate::Session::read_sampled_source`] validates it, decoding each state
//! as it is produced. The validator still reads only the JSON form, so its
//! independence from the producer is the same as for a document.
//!
//! ```
//! use dive_trace::stream::{StateSource, Streamed, write_document};
//! use dive_trace::TraceDoc;
//! use serde_json::json;
//!
//! let doc = TraceDoc::behavior(vec![json!({"x": 0}), json!({"x": 1})])?;
//! let mut text = Vec::new();
//! write_document(&doc.header(), &doc, &mut text)?;
//! assert_eq!(text, doc.to_json().to_string().into_bytes());
//! assert_eq!(StateSource::len(&doc), 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::io::{self, Write};

use serde_json::{Map, Value as J};

use crate::format::{Label, TraceDoc, state_json};

/// One state as a source produces it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Streamed {
    /// The values by variable name, in dive's JSON form.
    pub values: Map<String, J>,
    /// The label of the step into the state (`None` for the first).
    pub label: Option<Label>,
    /// The members drawn for the step's existentials
    /// ([`TraceDoc::witnesses`]).
    pub witnesses: Vec<J>,
}

/// The states of a document, produced in order on demand. A source may be
/// read more than once (validation replays a word when it retries Init), and
/// must produce the same states each time.
pub trait StateSource: Sync {
    /// The number of states.
    fn len(&self) -> usize;

    /// Whether there are no states.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Calls `each` with every state in order, stopping at its first error.
    ///
    /// # Errors
    /// The first error of `each`, or why a state cannot be produced.
    fn states(&self, each: &mut dyn FnMut(Streamed) -> Result<(), String>) -> Result<(), String>;
}

impl StateSource for TraceDoc {
    fn len(&self) -> usize {
        self.states.len()
    }

    fn states(&self, each: &mut dyn FnMut(Streamed) -> Result<(), String>) -> Result<(), String> {
        for (k, values) in self.states.iter().enumerate() {
            each(Streamed {
                values: values.clone(),
                label: self.labels.get(k).cloned().flatten(),
                witnesses: self.witnesses.get(k).cloned().unwrap_or_default(),
            })?;
        }
        Ok(())
    }
}

impl TraceDoc {
    /// This document without its states, labels and witnesses: the header a
    /// [`StateSource`] completes.
    #[must_use]
    pub fn header(&self) -> Self {
        Self {
            states: Vec::new(),
            labels: Vec::new(),
            witnesses: Vec::new(),
            ..self.clone()
        }
    }
}

/// Writes the document made of `header` and the states of `source`, exactly
/// as [`TraceDoc::to_json`] of the whole document would be written, holding
/// one state at a time.
///
/// # Errors
/// A write error, or the source's error as [`io::ErrorKind::InvalidData`].
pub fn write_document(
    header: &TraceDoc,
    source: &dyn StateSource,
    out: &mut dyn Write,
) -> io::Result<()> {
    // The envelope's keys are sorted, so `"states":[]` occurs once and the
    // states go between its brackets.
    let envelope = header.envelope(J::Array(Vec::new())).to_string();
    let marker = "\"states\":[]";
    let at = envelope
        .find(marker)
        .ok_or_else(|| io::Error::other("document envelope without states"))?;
    let split = at + marker.len() - 1;
    out.write_all(&envelope.as_bytes()[..split])?;
    let mut first = true;
    let mut failure = None;
    let produced = source.states(&mut |state| {
        let text = state_json(&state.values, state.label.as_ref(), &state.witnesses).to_string();
        let written = if first {
            out.write_all(text.as_bytes())
        } else {
            out.write_all(b",")
                .and_then(|()| out.write_all(text.as_bytes()))
        };
        first = false;
        written.map_err(|error| {
            let message = error.to_string();
            failure = Some(error);
            message
        })
    });
    if let Some(error) = failure {
        return Err(error);
    }
    produced.map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
    out.write_all(&envelope.as_bytes()[split..])
}

/// Counts the bytes written through it and hashes them, so a caller can size
/// or summarize a document without holding its text.
#[derive(Debug, Default)]
pub struct Measure {
    /// The bytes written.
    pub len: usize,
    hasher: sha2::Sha256,
}

impl Measure {
    /// The SHA-256 digest of the bytes written, in lower-case hexadecimal.
    #[must_use]
    pub fn sha256_hex(self) -> String {
        use sha2::Digest as _;
        self.hasher
            .finalize()
            .iter()
            .fold(String::new(), |mut out, byte| {
                use std::fmt::Write as _;
                let _ = write!(out, "{byte:02x}");
                out
            })
    }
}

impl Write for Measure {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        use sha2::Digest as _;
        self.len += bytes.len();
        self.hasher.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(bytes: &[u8]) -> String {
        let mut measure = Measure::default();
        let _ = measure.write_all(bytes);
        measure.sha256_hex()
    }

    #[test]
    fn digests_match_the_standard_vectors() {
        assert_eq!(
            digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        let mut measure = Measure::default();
        for chunk in million.chunks(997) {
            let _ = measure.write_all(chunk);
        }
        assert_eq!(measure.len, 1_000_000);
        assert_eq!(
            measure.sha256_hex(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
