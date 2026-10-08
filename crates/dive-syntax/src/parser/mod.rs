//! A recursive-descent port of SANY's parser.
//!
//! The structure is idiomatic Rust, but the *acceptance* follows SANY's
//! `JavaCC` grammar (`tla2sany/javacc/tla+.jj`, tlaplus commit `94d0c50`)
//! choice by choice, so that `dive` accepts exactly what SANY's parser
//! accepts. The SANY mechanisms ported here are:
//!
//! * `belchDEF`: before each definition, SANY inserts a fictitious
//!   `DEFBREAK` token into the token stream at the start of the next
//!   definition's left-hand side, found by scanning ahead to the next `==`.
//!   Definitions are only recognised where such a token sits (see
//!   [`Parser::belch_def`]).
//! * `JavaCC` lookahead: each choice uses the same token tests as the grammar.
//! * The operator stack (`OperatorStack.java`), which resolves precedence
//!   *ranges* and rejects conflicts (see `expr.rs`).
//! * Junction-list alignment (`JunctionListContext.java`).
//! * Proof step levels (`proofLevelStack` and friends, see `proof.rs`).
//!
//! The first error matches SANY's verdict. After an error the parser
//! recovers at the next module-level unit, so editors still get a full tree.

mod expr;
mod module;
mod proof;

use crate::diagnostics::{Code, Diagnostic};
#[allow(
    clippy::enum_glob_use,
    reason = "the grammar names over a hundred token and node kinds"
)]
use crate::kind::SyntaxKind::{self, *};
use crate::lexer::{LexFlag, LexToken};
use crate::ops::OpInfo;
use crate::tree::Raw;
use rowan::{TextRange, TextSize};

/// Maximum nesting of expressions, proofs and modules before giving up.
///
/// SANY is limited only by the Java stack; this keeps `dive` from
/// overflowing its own stack on adversarial input (see `docs/divergences.md`).
pub(crate) const MAX_DEPTH: u32 = 256;

/// Maximum height of one module unit's syntax tree.
///
/// rowan drops green trees recursively, so a very deep tree (such as a
/// chain of thousands of `+`) could overflow the stack when freed. SANY
/// accepts such input; `dive` refuses it explicitly (see `docs/divergences.md`).
pub(crate) const MAX_TREE_HEIGHT: u32 = 2048;

/// A position in SANY's token stream, which interleaves fictitious
/// `DEFBREAK` tokens with the real ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VPos {
    /// Index of a significant token (`n` is end of input).
    idx: u32,
    /// True if this is the fictitious `DEFBREAK` just before `idx`.
    virt: bool,
}

/// A fatal parse error: SANY's `ParseException`.
#[derive(Debug)]
pub(crate) struct PErr {
    at: VPos,
    code: Code,
    message: String,
    help: Option<String>,
}

pub(crate) type PResult<T> = Result<T, PErr>;

/// A significant token as the parser sees it.
#[derive(Debug, Clone, Copy)]
struct Tok {
    /// Index into the lexed token list.
    full: u32,
    start: u32,
    end: u32,
    /// SANY column.
    col: u32,
    /// True if this is the first significant token on its line.
    line_start: bool,
}

/// The parser state.
pub(crate) struct Parser<'s> {
    src: &'s str,
    toks: Vec<Tok>,
    /// Current kind of each significant token. SANY turns some keywords
    /// into identifiers in place (field names, module names).
    kinds: Vec<SyntaxKind>,
    /// `defbreak[i]`: a fictitious `DEFBREAK` precedes token `i`.
    defbreak: Vec<bool>,
    cur: VPos,
    /// Open junction lists: (bullet column, bullet kind).
    junctions: Vec<(u32, SyntaxKind)>,
    proof_depth: i32,
    proof_levels: Vec<i32>,
    /// SANY's global `lastOp`: the last operator token parsed.
    last_op: Option<&'static OpInfo>,
    /// SANY's global `FairnessHook`: argument count of the last bare
    /// `Id(args)` parsed by a reduced expression.
    fairness_hook: Option<usize>,
    depth: u32,
    diags: Vec<Diagnostic>,
}

/// Output of the parser.
pub(crate) struct ParseOutput {
    pub(crate) root: Raw,
    pub(crate) kinds: Vec<SyntaxKind>,
    pub(crate) sig: Vec<u32>,
    pub(crate) trailer_from: Option<u32>,
    pub(crate) diags: Vec<Diagnostic>,
}

pub(crate) fn is_infix_tok(k: SyntaxKind) -> bool {
    matches!(k, INFIX_OP | AND_OP | OR_OP | IN_OP | EQ_OP | MINUS_OP)
}

/// SANY's `PrefixOpToken`.
pub(crate) fn is_prefix_tok(k: SyntaxKind) -> bool {
    matches!(k, PREFIX_OP | BOX_OP | DIAMOND_OP)
}

/// SANY's `NEPrefixOpToken` (adds `-.`).
pub(crate) fn is_ne_prefix_tok(k: SyntaxKind) -> bool {
    is_prefix_tok(k) || k == NEG_DOT_OP
}

/// SANY's `isOp`: any operator token.
fn is_any_op(k: SyntaxKind) -> bool {
    k.is_operator()
}

/// SANY's `OpenStart`.
pub(crate) fn is_open_start(k: SyntaxKind) -> bool {
    matches!(
        k,
        CASE_KW
            | CHOOSE_KW
            | EXISTS_KW
            | FORALL_KW
            | IF_KW
            | LET_KW
            | TEMPORAL_EXISTS_KW
            | TEMPORAL_FORALL_KW
    )
}

/// FIRST(Expression) in SANY's grammar.
pub(crate) fn starts_expr(k: SyntaxKind) -> bool {
    k.is_operator()
        || is_open_start(k)
        || matches!(
            k,
            L_PAREN
                | L_BRACE
                | L_BRACK
                | L_ANGLE
                | WF_KW
                | SF_KW
                | STRING
                | NUMBER
                | IDENT
                | STEP_NUMBER
                | STEP_IMPLICIT
        )
}

/// FIRST(OpOrExpr): an operator argument.
pub(crate) fn starts_op_or_expr(k: SyntaxKind) -> bool {
    starts_expr(k) || k == LAMBDA_KW
}

/// SANY's `isFieldNameToken`: keywords that may be used as field and
/// module names.
fn is_field_name_kw(k: SyntaxKind, text: &str) -> bool {
    if k == PREFIX_OP {
        return matches!(
            text,
            "ENABLED" | "UNCHANGED" | "SUBSET" | "UNION" | "DOMAIN"
        );
    }
    k == UNDERSCORE
        || (k.is_keyword()
            && !matches!(
                k,
                EXISTS_KW | FORALL_KW | TEMPORAL_EXISTS_KW | TEMPORAL_FORALL_KW
            ))
}

/// A step token: SANY's `StepStartToken`.
pub(crate) fn is_step_start(k: SyntaxKind) -> bool {
    matches!(
        k,
        STEP_NUMBER | STEP_IMPLICIT | STEP_DOT | STEP_BARE | STEP_UNNUMBERED
    )
}

impl<'s> Parser<'s> {
    pub(crate) fn new(src: &'s str, lexed: &[LexToken]) -> Self {
        let mut toks = Vec::new();
        let mut kinds = Vec::new();
        let mut newline_since_last = true;
        for (i, t) in lexed.iter().enumerate() {
            if t.kind.is_trivia() {
                let text = src.get(t.start as usize..t.end as usize).unwrap_or("");
                if text.contains(['\n', '\r']) {
                    newline_since_last = true;
                }
                continue;
            }
            toks.push(Tok {
                full: u32::try_from(i).unwrap_or(u32::MAX),
                start: t.start,
                end: t.end,
                col: t.col,
                line_start: newline_since_last,
            });
            kinds.push(t.kind);
            newline_since_last = false;
        }
        let end = u32::try_from(src.len()).unwrap_or(u32::MAX);
        toks.push(Tok {
            full: u32::try_from(lexed.len()).unwrap_or(u32::MAX),
            start: end,
            end,
            col: 0,
            line_start: true,
        });
        kinds.push(EOF);
        let n = toks.len();
        Self {
            src,
            toks,
            kinds,
            defbreak: vec![false; n],
            cur: VPos {
                idx: 0,
                virt: false,
            },
            junctions: Vec::new(),
            proof_depth: -1,
            proof_levels: Vec::new(),
            last_op: None,
            fairness_hook: None,
            depth: 0,
            diags: Vec::new(),
        }
    }

    // ---- Token stream -------------------------------------------------------

    /// Index of the end-of-input token.
    fn eof(&self) -> u32 {
        u32::try_from(self.toks.len().saturating_sub(1)).unwrap_or(u32::MAX)
    }

    fn kind(&self, v: VPos) -> SyntaxKind {
        if v.virt {
            DEFBREAK_LIT
        } else {
            self.kinds.get(v.idx as usize).copied().unwrap_or(EOF)
        }
    }

    fn next_v(&self, v: VPos) -> VPos {
        if v.virt {
            VPos {
                idx: v.idx,
                virt: false,
            }
        } else if v.idx >= self.eof() {
            v
        } else {
            let j = v.idx + 1;
            VPos {
                idx: j,
                virt: self.defbreak.get(j as usize).copied().unwrap_or(false),
            }
        }
    }

    fn prev_v(&self, v: VPos) -> Option<VPos> {
        if !v.virt && self.defbreak.get(v.idx as usize).copied().unwrap_or(false) {
            return Some(VPos {
                idx: v.idx,
                virt: true,
            });
        }
        if v.idx == 0 {
            None
        } else {
            Some(VPos {
                idx: v.idx - 1,
                virt: false,
            })
        }
    }

    /// The `k`-th token ahead (1-based), like `JavaCC`'s `getToken(k)`.
    fn la(&self, k: usize) -> VPos {
        let mut v = self.cur;
        for _ in 1..k {
            v = self.next_v(v);
        }
        v
    }

    fn la_kind(&self, k: usize) -> SyntaxKind {
        self.kind(self.la(k))
    }

    fn peek(&self) -> SyntaxKind {
        self.kind(self.cur)
    }

    fn at(&self, k: SyntaxKind) -> bool {
        self.peek() == k
    }

    fn text(&self, idx: u32) -> &'s str {
        self.toks
            .get(idx as usize)
            .and_then(|t| self.src.get(t.start as usize..t.end as usize))
            .unwrap_or("")
    }

    fn text_v(&self, v: VPos) -> &'s str {
        if v.virt { "" } else { self.text(v.idx) }
    }

    fn col(&self, v: VPos) -> u32 {
        self.toks.get(v.idx as usize).map_or(0, |t| t.col)
    }

    /// SANY's `junctionListCtx.isAboveCurrent(column)`.
    fn above(&self, v: VPos) -> bool {
        self.junctions.last().is_none_or(|&(c, _)| c < self.col(v))
    }

    /// Consumes the current token and returns its index. For a fictitious
    /// `DEFBREAK` the index is that of the following real token.
    fn bump(&mut self) -> u32 {
        let v = self.cur;
        if v.virt {
            self.cur.virt = false;
        } else if v.idx < self.eof() {
            self.cur = self.next_v(v);
        }
        v.idx
    }

    /// Consumes a `DEFBREAK` (fictitious or the literal `-|-`).
    /// Returns the token index if it was a real token.
    fn bump_defbreak(&mut self) -> PResult<Option<u32>> {
        if !self.at(DEFBREAK_LIT) {
            return Err(self.expected("the start of a definition"));
        }
        let virt = self.cur.virt;
        let i = self.bump();
        Ok(if virt { None } else { Some(i) })
    }

    fn expect(&mut self, k: SyntaxKind, what: &str) -> PResult<u32> {
        if self.at(k) {
            Ok(self.bump())
        } else {
            Err(self.expected(what))
        }
    }

    /// Turns a keyword at the current position into an identifier when SANY
    /// would (`isFieldNameToken`).
    fn convert_field_name(&mut self) {
        let v = self.cur;
        if !v.virt
            && let Some(k) = self.kinds.get(v.idx as usize).copied()
            && is_field_name_kw(k, self.text(v.idx))
            && let Some(slot) = self.kinds.get_mut(v.idx as usize)
        {
            *slot = IDENT;
        }
    }

    // ---- Errors -------------------------------------------------------------

    fn describe(&self, v: VPos) -> String {
        if v.virt {
            return "the start of a definition".to_owned();
        }
        match self.kind(v) {
            EOF => "end of input".to_owned(),
            _ => format!("`{}`", self.text(v.idx)),
        }
    }

    #[allow(
        clippy::unused_self,
        reason = "a method so error sites read like `self.expected(..)`"
    )]
    fn err_at(&self, at: VPos, code: Code, message: impl Into<String>) -> PErr {
        PErr {
            at,
            code,
            message: message.into(),
            help: None,
        }
    }

    fn expected(&self, what: &str) -> PErr {
        let at = self.cur;
        let code = if self.kind(at) == LEX_ERROR {
            Code::UnexpectedCharacter
        } else {
            Code::UnexpectedToken
        };
        self.err_at(
            at,
            code,
            format!("expected {what}, found {}", self.describe(at)),
        )
    }

    fn range_of(&self, v: VPos) -> TextRange {
        let t = self.toks.get(v.idx as usize);
        let (s, e) = t.map_or((0, 0), |t| (t.start, t.end));
        if v.virt {
            TextRange::empty(TextSize::from(s))
        } else {
            TextRange::new(TextSize::from(s), TextSize::from(e))
        }
    }

    fn record(&mut self, e: PErr) {
        // A lexical error at this token is already reported by the lexer pass.
        if !e.at.virt
            && self.kind(e.at) == LEX_ERROR
            && matches!(e.code, Code::UnexpectedToken | Code::UnexpectedCharacter)
        {
            return;
        }
        let mut d = Diagnostic::error(e.code, self.range_of(e.at), e.message);
        d.help = e.help;
        self.diags.push(d);
    }

    /// Records a non-fatal error, like SANY's `PErrors.push`.
    fn soft_error(&mut self, at: VPos, code: Code, message: impl Into<String>) {
        let e = self.err_at(at, code, message);
        self.record(e);
    }

    fn enter(&mut self) -> PResult<()> {
        if self.depth >= MAX_DEPTH {
            return Err(self.err_at(
                self.cur,
                Code::NestingTooDeep,
                format!("nesting deeper than {MAX_DEPTH} levels is not supported"),
            ));
        }
        self.depth += 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // ---- belchDEF -----------------------------------------------------------

    /// SANY's `belchDEF`: scan ahead to the next `==` and insert a fictitious
    /// `DEFBREAK` before the start of that definition's left-hand side.
    ///
    /// The scan stops early at `THEOREM`, `PROPOSITION` (and `LEMMA`,
    /// `COROLLARY`), `ASSUMPTION` (and `AXIOM`), `====` and end of input.
    /// Returns the index of the `==` found, if any.
    fn belch_def(&mut self) -> Option<u32> {
        self.belch_def_scan().ok()
    }

    /// [`Parser::belch_def`], returning `Err(stop)` with the index of the
    /// token where the scan stopped when no `==` was found.
    fn belch_def_scan(&mut self) -> Result<u32, u32> {
        // The scan is a chain starting at the last consumed token (`None`
        // stands for `JavaCC`'s initial dummy token, whose kind is EOF).
        let head: Option<VPos> = self.prev_v(self.cur);
        let kind_of = |p: &Self, t: Option<VPos>| t.map_or(EOF, |v| p.kind(v));
        // Predecessor inside the chain; `None` means "no predecessor".
        let before = |p: &Self, t: Option<VPos>| -> Option<Option<VPos>> {
            if t == head {
                return None;
            }
            t.map(|v| p.prev_v(v))
        };
        let mut prev = head;
        let mut cur = self.cur;
        let mut next = self.next_v(cur);
        loop {
            let nk = self.kind(next);
            let ck = self.kind(cur);
            if matches!(
                nk,
                EOF | THEOREM_KW | PROPOSITION_KW | ASSUMPTION_KW | END_MODULE
            ) || matches!(ck, THEOREM_KW | PROPOSITION_KW)
            {
                return Err(next.idx);
            }
            if ck == DEF_EQ {
                let def_idx = cur.idx;
                let mut t = prev;
                let tk = kind_of(self, t);
                if matches!(tk, R_PAREN | R_BRACK) {
                    let mut depth = 1u32;
                    while let Some(p) = before(self, t) {
                        t = p;
                        let k = kind_of(self, t);
                        if matches!(k, L_PAREN | L_BRACK) {
                            if depth == 1 {
                                break;
                            }
                            depth -= 1;
                        }
                        if matches!(k, R_PAREN | R_BRACK) {
                            depth += 1;
                        }
                    }
                    match before(self, t) {
                        None => return Ok(def_idx),
                        Some(p) => {
                            if kind_of(self, p) == IDENT {
                                t = p;
                            }
                        }
                    }
                } else if tk == IDENT {
                    let identifier = t;
                    if let Some(p) = before(self, t) {
                        let pk = kind_of(self, p);
                        let pp_ident = before(self, p).is_some_and(|pp| kind_of(self, pp) == IDENT);
                        if is_any_op(pk) && pk != POSTFIX_OP && (!is_infix_tok(pk) || pp_ident) {
                            t = p;
                            match before(self, t) {
                                Some(q) if kind_of(self, q) == LEFT_ARROW => t = identifier,
                                Some(q) if is_infix_tok(kind_of(self, t)) => t = q,
                                Some(_) | None => {}
                            }
                        }
                    }
                } else if is_any_op(tk)
                    && let Some(p) = before(self, t)
                {
                    t = p;
                }
                // Insert before t, unless a DEFBREAK is already there or t is
                // the already-consumed head (where SANY's insertion is lost).
                if t != head
                    && let Some(v) = t
                    && !v.virt
                {
                    let already = before(self, t).is_some_and(|p| kind_of(self, p) == DEFBREAK_LIT);
                    if !already {
                        if let Some(slot) = self.defbreak.get_mut(v.idx as usize) {
                            *slot = true;
                        }
                        if self.cur == v {
                            self.cur.virt = true;
                        }
                    }
                }
                return Ok(def_idx);
            }
            prev = Some(cur);
            cur = next;
            next = self.next_v(next);
        }
    }

    // ---- Recovery -----------------------------------------------------------

    /// Records `e`, then skips to the next plausible module-level unit.
    /// Returns an `ERROR` node holding every token from `start` up to there.
    fn recover(&mut self, e: PErr, start: u32, unit_col: u32) -> Raw {
        self.record(e);
        self.junctions.clear();
        self.proof_depth = -1;
        self.proof_levels.clear();
        let eof = self.eof();
        // Make progress: skip at least one token.
        let mut j = self.cur.idx.max(start + 1).min(eof);
        // Definition starts are found by belchDEF scans; `scanned_to` is
        // where the last scan ended, so each token is scanned once.
        let mut scanned_to = j;
        loop {
            if j >= eof {
                break;
            }
            let k = self.kinds.get(j as usize).copied().unwrap_or(EOF);
            let t = self.toks.get(j as usize).copied();
            let starts_line = t.is_some_and(|t| t.line_start && t.col <= unit_col);
            if matches!(k, END_MODULE | BEGIN_MODULE) {
                break;
            }
            if starts_line
                && matches!(
                    k,
                    VARIABLE_KW
                        | CONSTANT_KW
                        | RECURSIVE_KW
                        | THEOREM_KW
                        | PROPOSITION_KW
                        | ASSUMPTION_KW
                        | ASSUME_KW
                        | INSTANCE_KW
                        | LOCAL_KW
                        | USE_KW
                        | HIDE_KW
                        | SEPARATOR
                )
            {
                break;
            }
            // Look for the next definition start.
            if scanned_to <= j {
                let saved = self.cur;
                self.cur = VPos {
                    idx: j,
                    virt: false,
                };
                scanned_to = match self.belch_def_scan() {
                    Ok(def) | Err(def) => def.max(j) + 1,
                };
                self.cur = saved;
            }
            if starts_line && self.defbreak.get(j as usize).copied().unwrap_or(false) {
                break;
            }
            j += 1;
        }
        let mut node = Raw::new(ERROR);
        for i in start..j {
            node.tok(i);
        }
        self.cur = VPos {
            idx: j,
            virt: self.defbreak.get(j as usize).copied().unwrap_or(false),
        };
        node
    }

    // ---- Driver ---------------------------------------------------------------

    pub(crate) fn parse(mut self) -> ParseOutput {
        let mut root = Raw::new(SOURCE_FILE);
        // CompilationUnit: belchDEF(); [Prelude()] Module().
        self.belch_def();
        if self.at(PRAGMA_START) {
            let mut pragma = Raw::new(PRAGMA);
            pragma.tok(self.bump());
            while matches!(self.peek(), NUMBER | IDENT) {
                pragma.tok(self.bump());
            }
            root.node(pragma);
        }
        let mut trailer_from = None;
        if self.at(BEGIN_MODULE) {
            let (module, ended) = self.module();
            root.node(module);
            if ended {
                // Everything after the top-level `====` is ignored text.
                let last = self.cur.idx.saturating_sub(1);
                trailer_from = self.toks.get(last as usize).map(|t| t.full + 1);
            }
        } else {
            let at = self.cur;
            let msg = if self.kind(at) == EOF {
                "no `---- MODULE` header found".to_owned()
            } else {
                format!("expected `---- MODULE`, found {}", self.describe(at))
            };
            let e = self
                .err_at(at, Code::NoModule, msg)
                .with_help("a module starts with a line like `---- MODULE Name ----`");
            self.record(e);
        }
        // Any significant tokens not yet in the tree (only possible when no
        // module was found) are wrapped in an error node.
        if trailer_from.is_none() && self.cur.idx < self.eof() {
            let mut err = Raw::new(ERROR);
            for i in self.cur.idx..self.eof() {
                err.tok(i);
            }
            root.node(err);
        }
        let limit = trailer_from.map_or(self.eof(), |f| {
            u32::try_from(self.toks.iter().take_while(|t| t.full < f).count()).unwrap_or(u32::MAX)
        });
        let sig: Vec<u32> = self.toks.iter().map(|t| t.full).collect();
        let _ = limit;
        ParseOutput {
            root,
            kinds: self.kinds,
            sig,
            trailer_from,
            diags: self.diags,
        }
    }
}

impl PErr {
    fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// Lexer diagnostics for tokens before `trailer_from`.
pub(crate) fn lexer_diagnostics(lexed: &[LexToken], trailer_from: Option<u32>) -> Vec<Diagnostic> {
    let limit = trailer_from.map_or(lexed.len(), |f| f as usize);
    let mut out = Vec::new();
    for t in lexed.iter().take(limit) {
        let range = TextRange::new(TextSize::from(t.start), TextSize::from(t.end));
        match t.flag {
            LexFlag::Ok => {}
            LexFlag::BadChar => out.push(
                Diagnostic::error(
                    Code::UnexpectedCharacter,
                    range,
                    "this character cannot start any TLA+ token",
                )
                .with_help("remove it, or put it inside a string or comment"),
            ),
            LexFlag::BadString => out.push(
                Diagnostic::error(
                    Code::BadString,
                    range,
                    "malformed string literal: a bad escape or no closing quote on this line",
                )
                .with_help(r#"the only escapes are \" \\ \n \t \r \f"#),
            ),
            LexFlag::UnterminatedComment => out.push(
                Diagnostic::error(
                    Code::UnterminatedComment,
                    range,
                    "block comment `(*` is never closed",
                )
                .with_help("add a matching `*)`; block comments nest"),
            ),
        }
    }
    out
}
