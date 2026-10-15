//! Multi-line expressions that keep their alignment (port of `pcal.TLAExpr`).
//!
//! An expression is a sequence of lines of tokens with columns. Each line
//! after the first may be *anchored* to a token of an earlier line (the last
//! token at or to the left of the line's first column). When substitution
//! moves an anchor token, every line anchored to it moves by the same amount,
//! so TLA+ junction lists and other layout-sensitive constructs keep their
//! meaning. Substitution follows `TLAExpr.substituteForAll`: occurrences are
//! found left to right, record labels (`.f`, `[f |->`, `[f :`) are skipped,
//! and with `par` a multi-token replacement is parenthesized.

/// Token classes of `pcal.TLAToken`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Built-in operators, punctuation and keywords.
    Builtin,
    /// Numbers.
    Number,
    /// Strings; the text keeps its quotes.
    Str,
    /// Identifiers.
    Ident,
    /// `pc` and `stack` added by the translator; they get `[self]`.
    Added,
    /// The `defaultInitValue` placeholder, which is never renamed or primed.
    Other,
}

/// One token with its column relative to the expression.
#[derive(Clone, Debug)]
pub(crate) struct Tok {
    pub text: String,
    pub kind: Kind,
    pub col: i64,
    id: u32,
    /// Java UTF-16 widths for original-stage substitutions.
    utf16: bool,
}

impl Tok {
    fn width(&self) -> i64 {
        let width = if self.utf16 {
            self.text.encode_utf16().count()
        } else {
            self.text.chars().count()
        };
        i64::try_from(width).unwrap_or(i64::MAX / 4)
    }
}

/// A token anchor: the anchoring token and its column when last normalized.
#[derive(Clone, Copy, Debug)]
struct Anchor {
    id: u32,
    col: i64,
}

/// A (possibly multi-line) TLA+ expression.
#[derive(Clone, Debug, Default)]
pub(crate) struct Expr {
    pub lines: Vec<Vec<Tok>>,
    anchors: Vec<Option<Anchor>>,
    next: u32,
    /// Original-stage Tokenize preserves internal empty physical lines.
    preserve_gaps: bool,
}

const BUILTIN_WORDS: &[&str] = &[
    "ASSUME",
    "ASSUMPTION",
    "AXIOM",
    "BOOLEAN",
    "CASE",
    "CHOOSE",
    "DOMAIN",
    "ELSE",
    "ENABLED",
    "EXCEPT",
    "FALSE",
    "IF",
    "IN",
    "INSTANCE",
    "LET",
    "LOCAL",
    "OTHER",
    "SF_",
    "STRING",
    "SUBSET",
    "THEN",
    "THEOREM",
    "TRUE",
    "UNCHANGED",
    "UNION",
    "VARIABLES",
    "WF_",
    "WITH",
    "_",
];

/// Classifies a source token the way `pcal.Tokenize` does.
pub(crate) fn classify(text: &str) -> Kind {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return Kind::Builtin;
    };
    if first == '"' {
        return Kind::Str;
    }
    if first == '\\'
        && text
            .chars()
            .nth(1)
            .is_some_and(|c| matches!(c, 'b' | 'B' | 'o' | 'O' | 'h' | 'H'))
        && text.chars().nth(2).is_some_and(|c| c.is_ascii_hexdigit())
    {
        return Kind::Number;
    }
    if first.is_ascii_digit() {
        return if text.chars().all(|c| c.is_ascii_digit() || c == '.') {
            Kind::Number
        } else {
            Kind::Ident
        };
    }
    if first.is_alphabetic() || first == '_' {
        if BUILTIN_WORDS.contains(&text) {
            return Kind::Builtin;
        }
        if text.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Kind::Ident;
        }
    }
    Kind::Builtin
}

impl Expr {
    /// An empty expression.
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    fn tok(&mut self, text: &str, kind: Kind, col: i64) -> Tok {
        let id = self.next;
        self.next += 1;
        Tok {
            text: text.to_owned(),
            kind,
            col,
            id,
            utf16: self.preserve_gaps,
        }
    }

    /// Builds an expression from `(text, kind, line, column)` source tokens.
    pub(crate) fn from_tokens(tokens: &[(String, Kind, usize, usize)]) -> Self {
        Self::from_source_tokens(tokens, false)
    }

    /// Source expressions before generation, retaining internal line gaps.
    pub(crate) fn from_original_tokens(tokens: &[(String, Kind, usize, usize)]) -> Self {
        Self::from_source_tokens(tokens, true)
    }

    fn from_source_tokens(tokens: &[(String, Kind, usize, usize)], preserve_gaps: bool) -> Self {
        let mut expr = Self {
            preserve_gaps,
            ..Self::default()
        };
        let mut current_line: Option<usize> = None;
        for (text, kind, line, col) in tokens {
            if current_line != Some(*line) {
                if preserve_gaps && let Some(previous) = current_line {
                    for _ in previous.saturating_add(1)..*line {
                        expr.lines.push(Vec::new());
                    }
                }
                expr.lines.push(Vec::new());
                current_line = Some(*line);
            }
            let tok = expr.tok(text, *kind, i64::try_from(*col).unwrap_or(0));
            if let Some(last) = expr.lines.last_mut() {
                last.push(tok);
            }
        }
        expr.normalize();
        expr
    }

    /// A one-line expression of tokens separated by single spaces
    /// (`PcalTranslate.TokVectorToExpr`).
    pub(crate) fn line(tokens: &[(&str, Kind)]) -> Self {
        let mut expr = Self::default();
        let mut col = 0;
        let mut line = Vec::new();
        for (text, kind) in tokens {
            let tok = expr.tok(text, *kind, col);
            col += tok.width() + 1;
            line.push(tok);
        }
        if !line.is_empty() {
            expr.lines.push(line);
        }
        expr.normalize();
        expr
    }

    /// Parses generated TLA+ text, classifying tokens; `added` names become
    /// [`Kind::Added`] wherever they occur.
    pub(crate) fn parse(text: &str, added: &[&str]) -> Self {
        let mut tokens = Vec::new();
        let mut line = 0;
        let mut line_start = 0;
        for raw in dive_syntax::tokens::tokenize_spec(text) {
            let start = usize::from(raw.range.start());
            let end = usize::from(raw.range.end());
            let piece = &text[start..end];
            if raw.kind.is_trivia() {
                for (offset, ch) in piece.char_indices() {
                    if ch == '\n' {
                        line += 1;
                        line_start = start + offset + 1;
                    }
                }
                continue;
            }
            let mut kind = classify(piece);
            if added.contains(&piece) {
                kind = Kind::Added;
            }
            let col = text[line_start..start].chars().count();
            tokens.push((piece.to_owned(), kind, line, col));
        }
        Self::from_tokens(&tokens)
    }

    /// The `defaultInitValue` placeholder expression.
    pub(crate) fn default_init() -> Self {
        Self::line(&[("defaultInitValue", Kind::Other)])
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.iter().all(Vec::is_empty)
    }

    pub(crate) fn is_one_token(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].len() == 1
    }

    pub(crate) fn first(&self) -> Option<&Tok> {
        self.lines.iter().flatten().next()
    }

    /// All tokens in order.
    pub(crate) fn tokens(&self) -> impl Iterator<Item = &Tok> {
        self.lines.iter().flatten()
    }

    /// `TLAExpr.normalize`: drops empty lines at the ends, moves the
    /// leftmost line start to column 0 and recomputes the anchors.
    pub(crate) fn normalize(&mut self) {
        if self.preserve_gaps {
            let start = self
                .lines
                .iter()
                .position(|line| !line.is_empty())
                .unwrap_or(self.lines.len());
            self.lines.drain(..start);
            while self.lines.last().is_some_and(Vec::is_empty) {
                self.lines.pop();
            }
        } else {
            self.lines.retain(|line| !line.is_empty());
        }
        let min = self
            .lines
            .iter()
            .filter_map(|line| line.first().map(|t| t.col))
            .min()
            .unwrap_or(0);
        for tok in self.lines.iter_mut().flatten() {
            tok.col -= min;
        }
        let mut anchors = Vec::with_capacity(self.lines.len());
        for i in 0..self.lines.len() {
            let Some(first) = self.lines[i].first() else {
                anchors.push(None);
                continue;
            };
            let first_col = first.col;
            let mut anchor = None;
            for j in (0..i).rev() {
                let candidate = &self.lines[j];
                if candidate
                    .first()
                    .is_some_and(|first| first.col <= first_col)
                {
                    let mut k = 0;
                    while k + 1 < candidate.len() && candidate[k + 1].col <= first_col {
                        k += 1;
                    }
                    anchor = Some(Anchor {
                        id: candidate[k].id,
                        col: candidate[k].col,
                    });
                    break;
                }
            }
            anchors.push(anchor);
        }
        self.anchors = anchors;
    }

    /// `TLAExpr.renormalize`: shifts every line whose anchor moved.
    fn renormalize(&mut self) {
        for i in 0..self.lines.len() {
            let Some(anchor) = self.anchors.get(i).copied().flatten() else {
                continue;
            };
            let now = self.lines[..i]
                .iter()
                .flatten()
                .find(|t| t.id == anchor.id)
                .map(|t| t.col);
            let Some(now) = now else {
                continue;
            };
            let shift = now - anchor.col;
            if shift != 0 {
                for tok in &mut self.lines[i] {
                    tok.col += shift;
                }
            }
            if let Some(Some(a)) = self.anchors.get_mut(i) {
                a.col = now;
            }
        }
    }

    /// Positions `(line, index)` of identifier occurrences of `ids`, skipping
    /// record labels (`TLAExpr.findNextInstanceIn`).
    fn instances(&self, ids: &[&str]) -> Vec<(usize, usize, usize)> {
        let flat: Vec<(usize, usize, &Tok)> = self
            .lines
            .iter()
            .enumerate()
            .flat_map(|(l, line)| line.iter().enumerate().map(move |(i, t)| (l, i, t)))
            .collect();
        let mut found = Vec::new();
        let mut i = 0;
        while i < flat.len() {
            let tok = flat[i].2;
            if tok.kind == Kind::Str {
                i += 1;
            } else if tok.text == "." {
                i += 2;
            } else if (tok.text == "[" || tok.text == ",")
                && flat.get(i + 2).is_some_and(|(_, _, t)| {
                    t.kind != Kind::Str && matches!(t.text.as_str(), ":" | "|->" | "↦")
                })
            {
                i += 3;
            } else if let Some(which) = ids.iter().position(|id| *id == tok.text) {
                found.push((flat[i].0, flat[i].1, which));
                i += 1;
            } else {
                i += 1;
            }
        }
        found
    }

    /// Simultaneously substitutes `subs[k].1` for identifier `subs[k].0`
    /// (`TLAExpr.substituteForAll`). With `par`, multi-token replacements are
    /// parenthesized unless the whole expression is the replaced token.
    pub(crate) fn substitute(&self, subs: &[(String, Expr)], par: bool) -> Expr {
        if subs.is_empty() {
            return self.clone();
        }
        let ids: Vec<&str> = subs.iter().map(|(id, _)| id.as_str()).collect();
        if self.is_one_token() {
            let tok = &self.lines[0][0];
            if tok.kind != Kind::Str
                && let Some(k) = ids.iter().position(|id| *id == tok.text)
            {
                let mut replaced = subs[k].1.clone();
                replaced.normalize();
                return replaced;
            }
            return self.clone();
        }
        let found = self.instances(&ids);
        if found.is_empty() {
            return self.clone();
        }
        let mut out = Expr {
            lines: Vec::new(),
            anchors: Vec::new(),
            next: self.next,
            preserve_gaps: self.preserve_gaps,
        };
        let mut hits = found.into_iter().peekable();
        for (l, line) in self.lines.iter().enumerate() {
            let mut current: Vec<Tok> = Vec::new();
            out.anchors.push(self.anchors.get(l).copied().flatten());
            let mut shift: i64 = 0;
            for (i, tok) in line.iter().enumerate() {
                let hit = hits.next_if(|(hl, hi, _)| *hl == l && *hi == i);
                let Some((_, _, which)) = hit else {
                    let mut moved = tok.clone();
                    moved.col += shift;
                    current.push(moved);
                    continue;
                };
                let base = tok.col + shift;
                let mut replacement = subs[which].1.clone();
                replacement.normalize();
                // Re-identify the replacement's tokens inside `out`.
                let offset = out.next;
                out.next += replacement.next.max(1);
                for t in replacement.lines.iter_mut().flatten() {
                    t.id += offset;
                }
                for a in replacement.anchors.iter_mut().flatten() {
                    a.id += offset;
                }
                if replacement.is_one_token() {
                    let mut new = tok.clone();
                    new.col = base;
                    new.text.clone_from(&replacement.lines[0][0].text);
                    new.kind = replacement.lines[0][0].kind;
                    shift += new.width() - tok.width();
                    current.push(new);
                    continue;
                }
                if replacement.is_empty() {
                    continue;
                }
                let indent = base + i64::from(par);
                let mut rlines = replacement.lines.into_iter();
                let ranchors = replacement.anchors;
                if par {
                    let mut paren = tok.clone();
                    paren.col = base;
                    paren.text = "(".into();
                    paren.kind = Kind::Builtin;
                    current.push(paren);
                }
                let mut first_line = rlines.next().unwrap_or_default();
                let mut keep_id = !par;
                for mut t in first_line.drain(..) {
                    t.col += indent;
                    if keep_id {
                        // The replaced token object takes the first token's text.
                        t.id = tok.id;
                        keep_id = false;
                    }
                    current.push(t);
                }
                for (index, rline) in rlines.enumerate() {
                    out.lines.push(std::mem::take(&mut current));
                    out.anchors.push(ranchors.get(index + 1).copied().flatten());
                    current = rline;
                }
                let end = current.last().map_or(base, |t| t.col + t.width());
                let mut after = end;
                if par {
                    let close = out.tok(")", Kind::Builtin, end);
                    after += 1;
                    current.push(close);
                }
                // Keep the original spacing to the rest of the line.
                let original_end = tok.col + tok.width();
                shift = after - original_end;
            }
            out.lines.push(current);
        }
        out.renormalize();
        out
    }

    /// `TLAExpr.prepend`: puts `other` in front, `spaces` after its last token.
    pub(crate) fn prepend(&mut self, other: &Expr, spaces: i64) {
        if other.is_empty() {
            return;
        }
        if self.is_empty() {
            *self = other.clone();
            return;
        }
        let mut other = other.clone();
        let offset = self.next;
        self.next += other.next.max(1);
        for t in other.lines.iter_mut().flatten() {
            t.id += offset;
        }
        for a in other.anchors.iter_mut().flatten() {
            a.id += offset;
        }
        let last = other.lines.len() - 1;
        let incr = other.lines[last]
            .last()
            .map_or(0, |t| t.col + t.width() + spaces);
        let first = std::mem::take(&mut self.lines[0]);
        let mut merged = std::mem::take(&mut other.lines[last]);
        for mut t in first {
            t.col += incr;
            merged.push(t);
        }
        let mut lines = other.lines;
        lines[last] = merged;
        lines.extend(self.lines.drain(1..));
        let mut anchors = other.anchors;
        anchors.extend(self.anchors.iter().skip(1).copied());
        self.lines = lines;
        self.anchors = anchors;
        self.renormalize();
    }

    /// Appends a token at `spaces` columns after the end of the last line.
    pub(crate) fn push(&mut self, text: &str, kind: Kind, spaces: i64) {
        let col = self
            .lines
            .last()
            .and_then(|l| l.last())
            .map_or(0, |t| t.col + t.width() + spaces);
        let tok = self.tok(text, kind, col);
        if let Some(line) = self.lines.last_mut() {
            line.push(tok);
        } else {
            self.lines.push(vec![tok]);
            self.anchors.push(None);
        }
    }

    /// Projected rendered bytes, including physical token gaps, without allocation.
    pub(crate) fn rendered_bytes(&self) -> usize {
        let min = self
            .lines
            .iter()
            .filter_map(|line| line.first().map(|t| t.col))
            .min()
            .unwrap_or(0);
        let mut bytes = self.lines.len().saturating_sub(1);
        for line in &self.lines {
            let mut at = min;
            for (index, tok) in line.iter().enumerate() {
                let wanted = tok.col.saturating_sub(at);
                let gap = if index == 0 || wanted >= 0 {
                    wanted.max(0)
                } else {
                    1
                };
                bytes = bytes
                    .saturating_add(usize::try_from(gap).unwrap_or(usize::MAX))
                    .saturating_add(tok.text.len());
                at = at.saturating_add(gap).saturating_add(tok.width());
            }
        }
        bytes
    }

    /// Text lines; columns are relative to the leftmost line start
    /// (`TLAExpr.toStringVector` after normalization).
    pub(crate) fn to_lines(&self) -> Vec<String> {
        let min = self
            .lines
            .iter()
            .filter_map(|line| line.first().map(|t| t.col))
            .min()
            .unwrap_or(0);
        let mut result = Vec::with_capacity(self.lines.len());
        for line in &self.lines {
            let mut text = String::new();
            let mut at = min;
            for (index, tok) in line.iter().enumerate() {
                let wanted = tok.col - at;
                // A negative gap (an overlap after a shrinking substitution)
                // must still separate the tokens.
                let gap = if index == 0 || wanted >= 0 {
                    wanted.max(0)
                } else {
                    1
                };
                text.extend(std::iter::repeat_n(' ', usize::try_from(gap).unwrap_or(0)));
                text.push_str(&tok.text);
                at += gap + tok.width();
            }
            result.push(text);
        }
        if result.is_empty() {
            result.push(String::new());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitution_keeps_junction_alignment() {
        let e = Expr::parse("/\\ x = 1\n/\\ y = x", &[]);
        let sub = Expr::parse("x'", &[]);
        let out = e.substitute(&[("x".into(), sub)], false);
        assert_eq!(out.to_lines(), ["/\\ x' = 1", "/\\ y = x'"]);
    }

    #[test]
    fn anchored_lines_follow_their_anchor() {
        let e = Expr::parse("f(x, /\\ a\n     /\\ b)", &[]);
        let sub = Expr::parse("x[self]", &[]);
        let out = e.substitute(&[("x".into(), sub)], false);
        assert_eq!(out.to_lines(), ["f(x[self], /\\ a", "           /\\ b)"]);
    }

    #[test]
    fn record_labels_are_not_substituted() {
        let e = Expr::parse("[x |-> x, y : x] /\\ r.x", &[]);
        let sub = Expr::parse("z", &[]);
        let out = e.substitute(&[("x".into(), sub)], true);
        assert_eq!(out.to_lines(), ["[x |-> z, y : z] /\\ r.x"]);
    }

    #[test]
    fn macro_arguments_are_parenthesized() {
        let e = Expr::parse("a + b", &[]);
        let sub = Expr::parse("c - d", &[]);
        let out = e.substitute(&[("a".into(), sub)], true);
        assert_eq!(out.to_lines(), ["(c - d) + b"]);
        let alone =
            Expr::parse("a", &[]).substitute(&[("a".into(), Expr::parse("c - d", &[]))], true);
        assert_eq!(alone.to_lines(), ["c - d"]);
    }

    #[test]
    fn prepend_joins_lines() {
        let mut e = Expr::parse("S", &[]);
        e.prepend(&Expr::parse("[CHOOSE self \\in", &[]), 1);
        assert_eq!(e.to_lines(), ["[CHOOSE self \\in S"]);
    }
}
