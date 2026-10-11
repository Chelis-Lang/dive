//! Port of TLC's value pretty-printer, `tlc2.pprint` (`PrettyPrint.mypp`, `Parse`,
//! `Format`, `Node`; tlaplus `94d0c50`).
//!
//! TLC prints a value by re-parsing its single-line `toString` form and breaking it
//! over lines of at most [`WIDTH`] columns. The parser understands only sets,
//! sequences, records, functions, `SUBSET`, intervals of non-negative integers, and
//! constants made of letters and digits; on anything else (for example a model value
//! with `_` in its name, a negative interval, or a string with an escaped quote) it
//! gives up and the value is printed on one line. The port keeps every such quirk.
//! Positions and widths count UTF-16 code units, as Java does.

use std::fmt::Write as _;

/// TLC's default width (`tlc2.value.Values.width`, 80).
pub const WIDTH: usize = 80;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Constant,
    Set,
    Sequence,
    Record,
    RecordPair,
    Function,
    FunctionPair,
    Subset,
}

#[derive(Debug)]
struct Node {
    kind: Kind,
    first: usize,
    last: usize,
    children: Vec<Node>,
}

impl Node {
    fn len(&self) -> i64 {
        i64::try_from(self.last).unwrap_or(i64::MAX) - i64::try_from(self.first).unwrap_or(0) + 1
    }
}

/// A parse failure (TLC's `ParseException` or `StringIndexOutOfBoundsException`).
struct Fail;

type PResult<T> = Result<T, Fail>;

struct Parser<'a> {
    s: &'a [u16],
    /// Current nesting; deeper input fails to parse, so it prints on one line.
    depth: std::cell::Cell<u32>,
}

fn unit_char(u: u16) -> Option<char> {
    char::from_u32(u32::from(u))
}

/// `Character.isWhitespace(char)`.
fn is_ws(u: u16) -> bool {
    match u {
        0x09..=0x0D | 0x1C..=0x1F | 0x20 => true,
        0xA0 | 0x2007 | 0x202F => false,
        _ => unit_char(u).is_some_and(|c| !c.is_ascii() && c.is_whitespace()),
    }
}

/// `Character.isDigit(char)`: exact for ASCII; Unicode numeric otherwise.
fn is_digit(u: u16) -> bool {
    unit_char(u).is_some_and(|c| {
        if c.is_ascii() {
            c.is_ascii_digit()
        } else {
            c.is_numeric()
        }
    })
}

fn is_letter_or_digit(u: u16) -> bool {
    unit_char(u).is_some_and(crate::print::java_is_letter_or_digit)
}

impl Parser<'_> {
    fn at(&self, i: usize) -> PResult<u16> {
        self.s.get(i).copied().ok_or(Fail)
    }

    fn region(&self, i: usize, pat: &str) -> bool {
        let pat: Vec<u16> = pat.encode_utf16().collect();
        self.s
            .get(i..i + pat.len())
            .is_some_and(|w| w == pat.as_slice())
    }

    fn skip(&self, mut i: usize) -> PResult<usize> {
        while is_ws(self.at(i)?) {
            i += 1;
        }
        Ok(i)
    }

    fn parse(&self, index: usize) -> PResult<Node> {
        let d = self.depth.get();
        if d >= crate::depth::MAX_DEPTH {
            return Err(Fail);
        }
        self.depth.set(d + 1);
        let r = self.parse_at(index);
        self.depth.set(d);
        r
    }

    fn parse_at(&self, index: usize) -> PResult<Node> {
        if index >= self.s.len() {
            return Err(Fail);
        }
        let index = self.skip(index)?;
        match self.at(index)? {
            0x7B => self.list(index, Kind::Set, 1, 0x7D, false),
            0x3C => self.list(index, Kind::Sequence, 2, 0x3E, true),
            0x5B => self.pairs(index, Kind::Record),
            0x28 => self.pairs(index, Kind::Function),
            _ if self.region(index, "SUBSET") => {
                let last = self.skip(index + 6)?;
                let elt = self.parse(last)?;
                Ok(Node {
                    kind: Kind::Subset,
                    first: index,
                    last: elt.last,
                    children: vec![elt],
                })
            }
            _ => self.constant(index),
        }
    }

    /// `parseSet` and `parseSequence`.
    fn list(
        &self,
        first: usize,
        kind: Kind,
        open: usize,
        close: u16,
        double: bool,
    ) -> PResult<Node> {
        let mut last = self.skip(first + open)?;
        let mut children = Vec::new();
        while self.at(last)? != close {
            let elt = self.parse(last)?;
            last = self.skip(elt.last + 1)?;
            children.push(elt);
            if self.at(last)? == u16::from(b',') {
                last += 1;
            }
            last = self.skip(last)?;
        }
        if double {
            last += 1;
        }
        Ok(Node {
            kind,
            first,
            last,
            children,
        })
    }

    /// `parseRecord` and `parseFunction` with their pair parsers.
    fn pairs(&self, first: usize, kind: Kind) -> PResult<Node> {
        let (close, sep, div, pair_kind) = if kind == Kind::Record {
            (u16::from(b']'), None, "|->", Kind::RecordPair)
        } else {
            (u16::from(b')'), Some("@@"), ":>", Kind::FunctionPair)
        };
        let mut last = self.skip(first + 1)?;
        let mut children = Vec::new();
        while self.at(last)? != close {
            let pair_first = last;
            let key = self.parse(last)?;
            let mut l = self.skip(key.last + 1)?;
            if !self.region(l, div) {
                return Err(Fail);
            }
            l = self.skip(l + div.len())?;
            let value = self.parse(l)?;
            let pair = Node {
                kind: pair_kind,
                first: pair_first,
                last: value.last,
                children: vec![key, value],
            };
            last = self.skip(pair.last + 1)?;
            children.push(pair);
            match sep {
                None => {
                    if self.at(last)? == u16::from(b',') {
                        last += 1;
                    }
                }
                Some(sep) => {
                    if self.region(last, sep) {
                        last += sep.len();
                    }
                }
            }
            last = self.skip(last)?;
        }
        Ok(Node {
            kind,
            first,
            last,
            children,
        })
    }

    fn constant(&self, index: usize) -> PResult<Node> {
        let first = self.skip(index)?;
        let mut last = first;
        let node = |last| Node {
            kind: Kind::Constant,
            first,
            last,
            children: Vec::new(),
        };
        let c = self.at(first)?;
        if c == u16::from(b'"') {
            loop {
                last += 1;
                if self.at(last)? == u16::from(b'"') {
                    break;
                }
            }
            return Ok(node(last));
        }
        if is_digit(c)
            && let Some(end) = self.interval(index)
        {
            return Ok(node(end));
        }
        let len = self.s.len();
        if c == u16::from(b'-') {
            last += 1;
            while is_digit(self.at(last)?) {
                last += 1;
                if last == len {
                    break;
                }
            }
            return Ok(node(last - 1));
        }
        if is_letter_or_digit(c) {
            while is_letter_or_digit(self.at(last)?) {
                last += 1;
                if last == len {
                    break;
                }
            }
            return Ok(node(last - 1));
        }
        Err(Fail)
    }

    /// `parseInterval`: `i..j` for non-negative `i` and `j`, or `None`.
    fn interval(&self, index: usize) -> Option<usize> {
        let s = self.s;
        let mut last = index;
        if !is_digit(*s.get(last)?) {
            return None;
        }
        while last < s.len() && is_digit(s[last]) {
            last += 1;
        }
        if !self.region(last, "..") {
            return None;
        }
        last += 2;
        if !is_digit(*s.get(last)?) {
            return None;
        }
        while is_digit(*s.get(last)?) {
            last += 1;
            if last == s.len() {
                break;
            }
        }
        Some(last - 1)
    }
}

struct Formatter<'a> {
    s: &'a [u16],
}

fn ulen(s: &str) -> i64 {
    i64::try_from(s.encode_utf16().count()).unwrap_or(i64::MAX)
}

impl Formatter<'_> {
    fn text(&self, n: &Node) -> String {
        String::from_utf16_lossy(&self.s[n.first..=n.last.min(self.s.len().saturating_sub(1))])
    }

    fn format(&self, n: &Node, width: i64, trailer: i64, pad: &str) -> PResult<String> {
        if n.len() <= width - trailer {
            return Ok(self.text(n));
        }
        match n.kind {
            Kind::Constant => Ok(self.text(n)),
            Kind::Set => self.simple(n, width, trailer, pad, "{ ", " }", "  ", ","),
            Kind::Sequence => self.simple(n, width, trailer, pad, "<< ", " >>", "   ", ","),
            Kind::Subset => self.simple(n, width, trailer, pad, "SUBSET ", " ", " ", ","),
            Kind::Function => self.pair_value(
                n,
                width,
                trailer,
                pad,
                ["( ", " )", "  ", " @@", " :>", "    "],
            ),
            Kind::Record => self.pair_value(
                n,
                width,
                trailer,
                pad,
                ["[ ", " ]", "  ", ",", " |->", "    "],
            ),
            Kind::RecordPair | Kind::FunctionPair => Err(Fail),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors Format.formatSimpleValue"
    )]
    fn simple(
        &self,
        n: &Node,
        width: i64,
        trailer: i64,
        pad: &str,
        open: &str,
        close: &str,
        inner: &str,
        sep: &str,
    ) -> PResult<String> {
        let w = width - ulen(open).max(ulen(inner));
        let t = trailer + ulen(close);
        let lp = format!("{pad}{inner}");
        let mut out = String::from(open);
        let count = n.children.len();
        for (i, c) in n.children.iter().enumerate() {
            if i + 1 < count {
                out.push_str(&self.format(c, w, ulen(sep), &lp)?);
                out.push_str(sep);
                out.push('\n');
                out.push_str(&lp);
            } else {
                out.push_str(&self.format(c, w, t, &lp)?);
            }
        }
        out.push_str(close);
        Ok(out)
    }

    fn pair_value(
        &self,
        n: &Node,
        width: i64,
        trailer: i64,
        pad: &str,
        d: [&str; 6],
    ) -> PResult<String> {
        let [open, close, inner, sep, div, divpad] = d;
        let cw = width - ulen(open).max(ulen(inner));
        let tw = trailer + ulen(close);
        let lp = format!("{pad}{inner}");
        let (sl, dl, dpl) = (ulen(sep), ulen(div), ulen(divpad));
        let lpd = format!("{lp}{divpad}");
        let mut out = String::from(open);
        let count = n.children.len();
        for (i, pair) in n.children.iter().enumerate() {
            let [arg, val] = pair.children.as_slice() else {
                return Err(Fail);
            };
            let last = i + 1 == count;
            let (al, vl) = (arg.len(), val.len());
            if !last {
                if al + dl + 1 + vl + sl <= cw {
                    let _ = write!(out, "{}{div} {}{sep}\n{lp}", self.text(arg), self.text(val));
                } else if al + dl <= cw {
                    let _ = write!(
                        out,
                        "{}{div}\n{lpd}{}{sep}\n{lp}",
                        self.text(arg),
                        self.format(val, cw - dpl, sl, &lpd)?
                    );
                } else {
                    let _ = write!(
                        out,
                        "{}{div}\n{lpd}{}{sep}\n{lp}",
                        self.format(arg, cw, tw + dl, &lp)?,
                        self.format(val, cw - dpl, sl, &lpd)?
                    );
                }
            } else if al + dl + 1 + vl <= cw - tw {
                let _ = write!(out, "{}{div} {}", self.text(arg), self.text(val));
            } else if al + dl <= cw {
                let _ = write!(
                    out,
                    "{}{div}\n{lpd}{}",
                    self.text(arg),
                    self.format(val, cw - dpl, tw, &lpd)?
                );
            } else {
                let _ = write!(
                    out,
                    "{}{div}\n{lpd}{}",
                    self.format(arg, cw, tw + dl, &lp)?,
                    self.format(val, cw - dpl, tw, &lpd)?
                );
            }
        }
        out.push_str(close);
        Ok(out)
    }
}

/// `Values.ppr(String)`: pretty-prints a value's `toString` form at width [`WIDTH`].
///
/// ```
/// use dive_value::print::ppr;
/// assert_eq!(ppr("{1, 2}"), "{1, 2}");
/// let long = format!("<<{}>>", vec!["1111111111"; 8].join(", "));
/// assert!(ppr(&long).starts_with("<< 1111111111,\n   1111111111,"));
/// ```
#[must_use]
pub fn ppr(value: &str) -> String {
    ppr_width(value, WIDTH)
}

/// `PrettyPrint.mypp(value, width)`: returns `value` unchanged when it does not parse.
#[must_use]
pub fn ppr_width(value: &str, width: usize) -> String {
    let units: Vec<u16> = value.encode_utf16().collect();
    let parser = Parser {
        s: &units,
        depth: std::cell::Cell::new(0),
    };
    let Ok(tree) = parser.parse(0) else {
        return value.to_owned();
    };
    if tree.last + 1 < units.len() {
        return value.to_owned();
    }
    let f = Formatter { s: &units };
    f.format(&tree, i64::try_from(width).unwrap_or(i64::MAX), 0, "")
        .unwrap_or_else(|_| value.to_owned())
}
