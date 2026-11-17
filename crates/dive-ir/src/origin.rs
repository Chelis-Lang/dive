//! Where IR nodes come from.
//!
//! An [`Origin`] records the byte range of the semantic node a definition or
//! expression was lowered from, SANY's line and column for both ends, and the
//! instantiated definition it was lowered under. Lines and columns follow
//! `JavaCC`'s `SimpleCharStream`, which SANY's locations and so TLC's
//! messages use: one column per UTF-16 code unit, a tab advancing to the next
//! multiple of 8, and `\n`, `\r` and `\r\n` each ending a line. The end
//! column is the column of the last character, inclusive.

use crate::ids::{DefId, FileId};
use serde::{Deserialize, Serialize};

/// A source file of the specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    /// Original module text, retained for source-aware TLC diagnostics.
    pub source: Box<str>,
    /// The name shown in diagnostics.
    pub display: String,
    /// The name of the file-level module it holds. SANY's locations, and so
    /// TLC's, name this module, also for nodes inside inner modules.
    pub module: String,
    /// Whether it is one of the embedded standard modules.
    pub standard: bool,
}

/// Where a node comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Origin {
    /// The file; `None` for nodes SANY creates without a location.
    pub file: Option<FileId>,
    /// The byte range in the file.
    pub range: (u32, u32),
    /// SANY's line and column of the first character.
    pub begin: (u32, u32),
    /// SANY's line and column of the last character.
    pub end: (u32, u32),
    /// The instantiated definition (`I!Op`) this node was lowered under, if
    /// any. Its own origin is the `INSTANCE` that created it.
    pub instance: Option<DefId>,
}

impl Origin {
    /// An origin with no location.
    pub const NONE: Origin = Origin {
        file: None,
        range: (0, 0),
        begin: (0, 0),
        end: (0, 0),
        instance: None,
    };
}

/// Line and column positions of one file, computed the way `JavaCC` does.
#[derive(Debug, Clone)]
pub(crate) struct Positions {
    /// Byte offset of the start of each line.
    line_starts: Vec<usize>,
}

impl Positions {
    pub(crate) fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes.get(i) {
                Some(b'\n') => line_starts.push(i + 1),
                Some(b'\r') => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    line_starts.push(i + 1);
                }
                _ => {}
            }
            i += 1;
        }
        Self { line_starts }
    }

    /// The 1-based line and `JavaCC` column of the character at `offset`.
    /// With `last`, the column of that character's last UTF-16 unit (for an
    /// inclusive end position).
    pub(crate) fn position(&self, text: &str, offset: usize, last: bool) -> (u32, u32) {
        let offset = offset.min(text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let start = self.line_starts.get(line).copied().unwrap_or(0);
        let mut column: u32 = 0;
        let mut result = 1;
        for (i, c) in text.get(start..).unwrap_or("").char_indices() {
            let at = start + i;
            if at > offset {
                break;
            }
            column = column.wrapping_add(1);
            if c == '\t' {
                column = column.saturating_sub(1);
                column = column.wrapping_add(8 - (column % 8));
            }
            let first = column;
            if c.len_utf16() == 2 {
                column = column.wrapping_add(1);
            }
            result = if last { column } else { first };
            if at == offset || matches!(c, '\n' | '\r') {
                break;
            }
        }
        (u32::try_from(line + 1).unwrap_or(u32::MAX), result)
    }
}

/// Formats SANY's location string, `line L, col C to line L2, col C2 of
/// module M` (`tla2sany/st/Location.java:344-360`).
#[must_use]
pub fn location_string(origin: &Origin, module: &str) -> String {
    format!(
        "line {}, col {} to line {}, col {} of module {module}",
        origin.begin.0, origin.begin.1, origin.end.0, origin.end.1
    )
}

#[cfg(test)]
mod tests {
    use super::Positions;

    #[test]
    fn columns_follow_javacc() {
        let text = "ab\n\tx\r\ny😀z\rw";
        let p = Positions::new(text);
        assert_eq!(p.position(text, 0, false), (1, 1));
        assert_eq!(p.position(text, 1, false), (1, 2));
        // A tab at the start of a line ends at column 8; the next character
        // is at column 9.
        assert_eq!(p.position(text, 4, false), (2, 9));
        // An astral character takes two columns.
        let y = text.find('y').unwrap_or(0);
        assert_eq!(p.position(text, y + 1, false), (3, 2));
        assert_eq!(p.position(text, y + 1, true), (3, 3));
        assert_eq!(p.position(text, y + 5, false), (3, 4));
        // A lone \r ends a line.
        assert_eq!(p.position(text, text.len() - 1, false), (4, 1));
    }
}
