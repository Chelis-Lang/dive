//! The lexer.
//!
//! It follows the lexical states of SANY's `JavaCC` grammar (`tla+.jj`):
//!
//! * `DEFAULT`: before the first module. Everything is skipped until
//!   `----` `-`* ` `* `MODULE`, or `--->` (which starts a pragma).
//! * `PRAGMA`: after `--->`. Numbers and identifiers are tokens; other
//!   characters are skipped until a module header.
//! * `SPEC`: the module text. Tokens are chosen by longest match. Equal-length
//!   matches go to the token defined first in `tla+.jj`, so keywords beat
//!   identifiers.
//!
//! Comments nest. Columns are counted exactly as `JavaCC`'s `SimpleCharStream`
//! counts them, because junction-list alignment depends on them: one column
//! per UTF-16 code unit, and a tab advances to the next multiple of 8.

#[allow(
    clippy::enum_glob_use,
    reason = "the grammar names over a hundred token and node kinds"
)]
use crate::kind::SyntaxKind::{self, *};

/// What went wrong with a token, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LexFlag {
    /// The token is fine.
    Ok,
    /// A character no token can start with.
    BadChar,
    /// A string literal with a bad escape or no closing quote on its line.
    BadString,
    /// A block comment still open at end of input.
    UnterminatedComment,
}

/// One lexed token, including trivia.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LexToken {
    pub(crate) kind: SyntaxKind,
    pub(crate) start: u32,
    pub(crate) end: u32,
    /// SANY's 1-based begin column.
    pub(crate) col: u32,
    pub(crate) flag: LexFlag,
}

/// Fixed-spelling tokens of the `SPEC` state, in `tla+.jj` definition order.
/// The index in this table is the tie-break priority among fixed tokens.
const KEYWORDS_AND_PUNCT: &[(&str, SyntaxKind)] = &[
    ("ACTION", ACTION_KW),
    ("ACTIONS", ACTION_KW),
    ("ASSUME", ASSUME_KW),
    ("[]ASSUME", BOX_ASSUME_KW),
    ("ASSUMPTION", ASSUMPTION_KW),
    ("AXIOM", ASSUMPTION_KW),
    ("CASE", CASE_KW),
    ("CHOOSE", CHOOSE_KW),
    ("CONSTANT", CONSTANT_KW),
    ("CONSTANTS", CONSTANT_KW),
    ("ELSE", ELSE_KW),
    ("EXCEPT", EXCEPT_KW),
    ("\\E", EXISTS_KW),
    ("\\exists", EXISTS_KW),
    ("∃", EXISTS_KW),
    ("EXTENDS", EXTENDS_KW),
    ("\\A", FORALL_KW),
    ("\\forall", FORALL_KW),
    ("∀", FORALL_KW),
    ("IF", IF_KW),
    ("INSTANCE", INSTANCE_KW),
    ("LET", LET_KW),
    ("IN", IN_KW),
    ("LOCAL", LOCAL_KW),
    ("MODULE", MODULE_KW),
    ("NEW", NEW_KW),
    ("OTHER", OTHER_KW),
    ("PROPOSITION", PROPOSITION_KW),
    ("LEMMA", PROPOSITION_KW),
    ("COROLLARY", PROPOSITION_KW),
    ("SF_", SF_KW),
    ("\\EE", TEMPORAL_EXISTS_KW),
    ("\\AA", TEMPORAL_FORALL_KW),
    ("THEN", THEN_KW),
    ("BY", BY_KW),
    ("ONLY", ONLY_KW),
    ("DEFINE", DEFINE_KW),
    ("DEF", DEF_KW),
    ("DEFS", DEF_KW),
    ("THEOREM", THEOREM_KW),
    ("USE", USE_KW),
    ("HIDE", HIDE_KW),
    ("HAVE", HAVE_KW),
    ("OBVIOUS", OBVIOUS_KW),
    ("OMITTED", OMITTED_KW),
    ("LAMBDA", LAMBDA_KW),
    ("TAKE", TAKE_KW),
    ("PROOF", PROOF_KW),
    ("PROVE", PROVE_KW),
    ("[]PROVE", BOX_PROVE_KW),
    ("QED", QED_KW),
    ("RECURSIVE", RECURSIVE_KW),
    ("STATE", STATE_KW),
    ("TEMPORAL", TEMPORAL_KW),
    ("TEMPORALS", TEMPORAL_KW),
    ("PICK", PICK_KW),
    ("WITNESS", WITNESS_KW),
    ("SUFFICES", SUFFICES_KW),
    ("VARIABLE", VARIABLE_KW),
    ("VARIABLES", VARIABLE_KW),
    ("WF_", WF_KW),
    ("WITH", WITH_KW),
    (",", COMMA),
    (":", COLON),
    ("::", COLON_COLON),
    ("∷", COLON_COLON),
    (".", DOT),
    ("_", UNDERSCORE),
    ("==", DEF_EQ),
    ("≜", DEF_EQ),
    ("(", L_PAREN),
    (")", R_PAREN),
    ("-|-", DEFBREAK_LIT),
    ("[", L_BRACK),
    ("]_", R_BRACK_UNDERSCORE),
    ("]", R_BRACK),
    ("{", L_BRACE),
    ("}", R_BRACE),
    ("<<", L_ANGLE),
    ("⟨", L_ANGLE),
    (">>_", R_ANGLE_UNDERSCORE),
    ("⟩_", R_ANGLE_UNDERSCORE),
    (">>", R_ANGLE),
    ("⟩", R_ANGLE),
    ("!", BANG),
    ("->", ARROW),
    ("→", ARROW),
    ("<-", LEFT_ARROW),
    ("←", LEFT_ARROW),
    ("|->", MAPS_TO),
    ("↦", MAPS_TO),
];

/// Operator tokens, in `tla+.jj` definition order (after literals).
pub(crate) const OPERATORS: &[(&str, SyntaxKind)] = &[
    ("'", POSTFIX_OP),
    ("^+", POSTFIX_OP),
    ("⁺", POSTFIX_OP),
    ("^*", POSTFIX_OP),
    ("^#", POSTFIX_OP),
    ("-.", NEG_DOT_OP),
    ("\\lnot", PREFIX_OP),
    ("¬", PREFIX_OP),
    ("\\neg", PREFIX_OP),
    ("~", PREFIX_OP),
    ("[]", BOX_OP),
    ("□", BOX_OP),
    ("<>", DIAMOND_OP),
    ("◇", DIAMOND_OP),
    ("ENABLED", PREFIX_OP),
    ("UNCHANGED", PREFIX_OP),
    ("SUBSET", PREFIX_OP),
    ("UNION", PREFIX_OP),
    ("DOMAIN", PREFIX_OP),
    ("//", INFIX_OP),
    ("/\\", AND_OP),
    ("∧", AND_OP),
    ("/=", INFIX_OP),
    ("/", INFIX_OP),
    ("\\/", OR_OP),
    ("∨", OR_OP),
    ("\\approx", INFIX_OP),
    ("\\asymp", INFIX_OP),
    ("\\bigcirc", INFIX_OP),
    ("\\bullet", INFIX_OP),
    ("\\cap", INFIX_OP),
    ("\\cdot", INFIX_OP),
    ("\\circ", INFIX_OP),
    ("\\cong", INFIX_OP),
    ("\\cup", INFIX_OP),
    ("\\div", INFIX_OP),
    ("\\doteq", INFIX_OP),
    ("\\equiv", INFIX_OP),
    ("\\geq", INFIX_OP),
    ("\\gg", INFIX_OP),
    ("\\in", IN_OP),
    ("∈", IN_OP),
    ("\\intersect", INFIX_OP),
    ("\\union", INFIX_OP),
    ("\\land", INFIX_OP),
    ("\\leq", INFIX_OP),
    ("\\ll", INFIX_OP),
    ("\\lor", INFIX_OP),
    ("\\o", INFIX_OP),
    ("\\odot", INFIX_OP),
    ("\\ominus", INFIX_OP),
    ("\\oplus", INFIX_OP),
    ("\\oslash", INFIX_OP),
    ("\\otimes", INFIX_OP),
    ("\\prec", INFIX_OP),
    ("\\preceq", INFIX_OP),
    ("\\propto", INFIX_OP),
    ("\\sim", INFIX_OP),
    ("\\simeq", INFIX_OP),
    ("\\sqcap", INFIX_OP),
    ("\\sqcup", INFIX_OP),
    ("\\sqsubset", INFIX_OP),
    ("\\sqsupset", INFIX_OP),
    ("\\sqsubseteq", INFIX_OP),
    ("\\sqsupseteq", INFIX_OP),
    ("\\star", INFIX_OP),
    ("\\subset", INFIX_OP),
    ("\\subseteq", INFIX_OP),
    ("\\succ", INFIX_OP),
    ("\\succeq", INFIX_OP),
    ("\\supset", INFIX_OP),
    ("\\supseteq", INFIX_OP),
    ("\\uplus", INFIX_OP),
    ("\\wr", INFIX_OP),
    ("\\", INFIX_OP),
    ("~>", INFIX_OP),
    ("=>", INFIX_OP),
    ("=<", INFIX_OP),
    ("=|", INFIX_OP),
    ("=", EQ_OP),
    ("##", INFIX_OP),
    ("#", INFIX_OP),
    ("^^", INFIX_OP),
    ("^", INFIX_OP),
    ("--", INFIX_OP),
    ("-|", INFIX_OP),
    ("-+->", INFIX_OP),
    ("-", MINUS_OP),
    ("**", INFIX_OP),
    ("*", INFIX_OP),
    ("++", INFIX_OP),
    ("+", INFIX_OP),
    ("<=>", INFIX_OP),
    ("<:", INFIX_OP),
    ("<=", INFIX_OP),
    ("<", INFIX_OP),
    (">=", INFIX_OP),
    (">", INFIX_OP),
    ("...", INFIX_OP),
    ("..", INFIX_OP),
    ("||", INFIX_OP),
    ("|", INFIX_OP),
    ("|-", INFIX_OP),
    ("|=", INFIX_OP),
    ("&&", INFIX_OP),
    ("&", INFIX_OP),
    ("$$", INFIX_OP),
    ("$", INFIX_OP),
    ("??", INFIX_OP),
    ("%%", INFIX_OP),
    ("%", INFIX_OP),
    ("@@", INFIX_OP),
    ("!!", INFIX_OP),
    (":>", INFIX_OP),
    (":=", INFIX_OP),
    ("::=", INFIX_OP),
    ("(+)", INFIX_OP),
    ("(-)", INFIX_OP),
    ("(.)", INFIX_OP),
    ("(/)", INFIX_OP),
    ("(\\X)", INFIX_OP),
    ("\\notin", INFIX_OP),
    ("≈", INFIX_OP),
    ("≔", INFIX_OP),
    ("≍", INFIX_OP),
    ("◯", INFIX_OP),
    ("⩴", INFIX_OP),
    ("●", INFIX_OP),
    ("∩", INFIX_OP),
    ("⋅", INFIX_OP),
    ("∘", INFIX_OP),
    ("≅", INFIX_OP),
    ("∪", INFIX_OP),
    ("÷", INFIX_OP),
    ("≐", INFIX_OP),
    ("‥", INFIX_OP),
    ("…", INFIX_OP),
    ("≡", INFIX_OP),
    ("‼", INFIX_OP),
    ("≥", INFIX_OP),
    ("≫", INFIX_OP),
    ("⇔", INFIX_OP),
    ("⇒", INFIX_OP),
    ("⫤", INFIX_OP),
    ("↝", INFIX_OP),
    ("≤", INFIX_OP),
    ("≪", INFIX_OP),
    ("⊣", INFIX_OP),
    ("≠", INFIX_OP),
    ("∉", INFIX_OP),
    ("⊙", INFIX_OP),
    ("⊖", INFIX_OP),
    ("⊕", INFIX_OP),
    ("⊘", INFIX_OP),
    ("⊗", INFIX_OP),
    ("⇸", INFIX_OP),
    ("≺", INFIX_OP),
    ("⪯", INFIX_OP),
    ("∝", INFIX_OP),
    ("⁇", INFIX_OP),
    ("⊨", INFIX_OP),
    ("⊢", INFIX_OP),
    ("∼", INFIX_OP),
    ("≃", INFIX_OP),
    ("⊓", INFIX_OP),
    ("⊔", INFIX_OP),
    ("⊏", INFIX_OP),
    ("⊑", INFIX_OP),
    ("⊐", INFIX_OP),
    ("⊒", INFIX_OP),
    ("⋆", INFIX_OP),
    ("⊂", INFIX_OP),
    ("⊆", INFIX_OP),
    ("≻", INFIX_OP),
    ("⪰", INFIX_OP),
    ("⊃", INFIX_OP),
    ("⊇", INFIX_OP),
    ("×", INFIX_OP),
    ("⊎", INFIX_OP),
    ("‖", INFIX_OP),
    ("≀", INFIX_OP),
    ("\\times", INFIX_OP),
    ("\\X", INFIX_OP),
];

/// Tie-break priorities of the regular-expression token families.
mod prio {
    pub(super) const BEGIN_MODULE: u32 = 0;
    pub(super) const SEPARATOR: u32 = 1;
    pub(super) const END_MODULE: u32 = 2;
    pub(super) const FIXED: u32 = 10;
    pub(super) const NUMBER: u32 = 500;
    pub(super) const STRING: u32 = 501;
    pub(super) const BAD_JUNCTION: u32 = 502;
    pub(super) const OPERATOR: u32 = 600;
    pub(super) const IDENT: u32 = 900;
    pub(super) const STEP: u32 = 901;
}

/// Tracks line and column exactly as `JavaCC`'s `SimpleCharStream` does.
struct ColTracker {
    pos: usize,
    column: u32,
    prev_cr: bool,
    prev_lf: bool,
}

impl ColTracker {
    fn new() -> Self {
        Self {
            pos: 0,
            column: 0,
            prev_cr: false,
            prev_lf: false,
        }
    }

    /// Feeds one UTF-16 code unit (as the char it belongs to).
    fn update(&mut self, c: char) {
        self.column = self.column.wrapping_add(1);
        if self.prev_lf {
            self.prev_lf = false;
            self.column = 1;
        } else if self.prev_cr {
            self.prev_cr = false;
            if c == '\n' {
                self.prev_lf = true;
            } else {
                self.column = 1;
            }
        }
        match c {
            '\r' => self.prev_cr = true,
            '\n' => self.prev_lf = true,
            '\t' => {
                self.column = self.column.saturating_sub(1);
                self.column = self.column.wrapping_add(8 - (self.column % 8));
            }
            _ => {}
        }
    }

    /// Processes characters up to and including the one at byte offset `at`
    /// and returns that character's column.
    fn column_at(&mut self, src: &str, at: usize) -> u32 {
        let mut col = self.column;
        while self.pos <= at {
            let Some(c) = src.get(self.pos..).and_then(|s| s.chars().next()) else {
                return self.column.wrapping_add(1);
            };
            let first_col = {
                self.update(c);
                self.column
            };
            if c.len_utf16() == 2 {
                self.update(c);
            }
            if self.pos == at {
                col = first_col;
            }
            self.pos += c.len_utf8();
        }
        col
    }
}

fn is_letter(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Length of the run of bytes satisfying `f` at the start of `s`.
fn run(s: &[u8], f: impl Fn(u8) -> bool) -> usize {
    s.iter().take_while(|&&b| f(b)).count()
}

/// `"----" ("-")* (" ")* "MODULE"`.
fn match_begin_module(s: &[u8]) -> usize {
    let dashes = run(s, |b| b == b'-');
    if dashes < 4 {
        return 0;
    }
    let spaces = run(&s[dashes..], |b| b == b' ');
    let rest = &s[dashes + spaces..];
    if rest.starts_with(b"MODULE") {
        dashes + spaces + 6
    } else {
        0
    }
}

/// The IDENTIFIER token: SANY's `CASE0|CASE1|CASE2|CASE3|CASE6|CASEN|"@"|NUMBER_SET`.
fn match_ident(s: &str) -> usize {
    let b = s.as_bytes();
    if b.first() == Some(&b'@') {
        return 1;
    }
    for set in ["ℕ", "ℤ", "ℝ"] {
        if s.starts_with(set) {
            return set.len();
        }
    }
    let w = run(b, is_word);
    if w == 0 {
        return 0;
    }
    let word = &b[..w];
    let has_letter = word.iter().any(|&c| is_letter(c));
    match word[0] {
        b'W' | b'S' => {
            if w == 1 || word[1] != b'F' {
                w
            } else if w == 2 || word[2] != b'_' {
                // "WF" alone, or "WF" followed by a letter or digit.
                w
            } else {
                2
            }
        }
        b'_' | b'0'..=b'9' => {
            if has_letter {
                w
            } else {
                0
            }
        }
        _ => w,
    }
}

/// SANY's `BAND`/`BOR` lexemes (`CASE1c`, `CASE2c`, `CASE6c` and their `b` forms).
fn match_bad_junction(s: &[u8]) -> usize {
    let v = run(s, |b| b.is_ascii_alphanumeric());
    if v == 0 {
        return 0;
    }
    let ok_start = match s[0] {
        b'0'..=b'9' => true,
        b'W' | b'S' => v >= 2 && s[1] != b'F',
        _ => is_letter(s[0]),
    };
    let rest = &s[v..];
    if ok_start && (rest.starts_with(b"./\\") || rest.starts_with(b".\\/")) {
        v + 3
    } else {
        0
    }
}

/// `NUMBER_LITERAL`: digits, or `\o`, `\b`, `\h` forms.
fn match_number(s: &[u8]) -> usize {
    let d = run(s, is_digit);
    if d > 0 {
        return d;
    }
    if s.first() != Some(&b'\\') || s.len() < 3 {
        return 0;
    }
    let digits = match s[1] {
        b'o' | b'O' => run(&s[2..], |b| (b'0'..=b'7').contains(&b)),
        b'b' | b'B' => run(&s[2..], |b| b == b'0' || b == b'1'),
        b'h' | b'H' => run(&s[2..], |b| b.is_ascii_hexdigit()),
        _ => 0,
    };
    if digits > 0 { digits + 2 } else { 0 }
}

/// `STRING_LITERAL`. Returns `Ok(len)` or `Err(len_of_error_token)`.
fn match_string(s: &[u8]) -> Result<usize, usize> {
    let mut i = 1;
    loop {
        match s.get(i) {
            None | Some(b'\n' | b'\r') => return Err(i),
            Some(b'"') => return Ok(i + 1),
            Some(b'\\') => {
                if let Some(b'n' | b't' | b'r' | b'f' | b'\\' | b'"') = s.get(i + 1) {
                    i += 2;
                } else {
                    // Bad escape: extend the error to the closing quote or end of line.
                    let tail = run(&s[i..], |b| b != b'"' && b != b'\n' && b != b'\r');
                    let end = i + tail;
                    return Err(if s.get(end) == Some(&b'"') {
                        end + 1
                    } else {
                        end
                    });
                }
            }
            Some(_) => i += 1,
        }
    }
}

/// The proof-step lexemes. Returns `(len, kind)` of the longest, if any.
fn match_step(s: &[u8]) -> Option<(usize, SyntaxKind)> {
    if s.first() != Some(&b'<') {
        return None;
    }
    let (lvl_len, implicit) = match s.get(1) {
        Some(b'+' | b'*') => (1, true),
        Some(c) if is_digit(*c) => (run(&s[1..], is_digit), false),
        _ => return None,
    };
    let base = 1 + lvl_len;
    if s.get(base) != Some(&b'>') {
        return None;
    }
    let base = base + 1;
    let rest = &s[base..];
    let mut best = (base, STEP_BARE, 3u8);
    let mut consider = |len: usize, kind: SyntaxKind, p: u8| {
        if len > best.0 || (len == best.0 && p < best.2) {
            best = (len, kind, p);
        }
    };
    let r = run(rest, is_word);
    if r > 0 {
        if implicit {
            let alnum = run(rest, |b| b.is_ascii_alphanumeric());
            if alnum > 0 {
                consider(base + alnum, STEP_IMPLICIT, 1);
            }
        } else {
            consider(base + r, STEP_NUMBER, 0);
        }
        let dots = run(&rest[r..], |b| b == b'.');
        if dots > 0 {
            consider(base + r + dots, STEP_DOT, 2);
        }
    }
    match rest.first() {
        Some(b'*' | b'-') => consider(
            base + 1 + run(&rest[1..], |b| b == b'.'),
            STEP_UNNUMBERED,
            4,
        ),
        Some(b'.') => consider(base + run(rest, |b| b == b'.'), STEP_UNNUMBERED, 4),
        _ => {}
    }
    Some((best.0, best.1))
}

/// A fixed-spelling token with its tie-break priority.
type Fixed = (&'static str, SyntaxKind, u32);

/// The fixed-spelling tokens, bucketed by their first byte.
fn fixed_buckets() -> &'static [Vec<Fixed>] {
    static BUCKETS: std::sync::OnceLock<Vec<Vec<Fixed>>> = std::sync::OnceLock::new();
    BUCKETS.get_or_init(|| {
        let mut b: Vec<Vec<Fixed>> = vec![Vec::new(); 256];
        let tables = [
            (KEYWORDS_AND_PUNCT, prio::FIXED),
            (OPERATORS, prio::OPERATOR),
        ];
        for (table, base) in tables {
            for (i, (text, kind)) in table.iter().enumerate() {
                let p = base + u32::try_from(i).unwrap_or(0);
                if let Some(&first) = text.as_bytes().first()
                    && let Some(slot) = b.get_mut(usize::from(first))
                {
                    slot.push((text, *kind, p));
                }
            }
        }
        b
    })
}

/// The best fixed-spelling token at the start of `s`: `(len, kind, priority)`.
fn match_fixed(s: &str) -> Option<(usize, SyntaxKind, u32)> {
    let first = *s.as_bytes().first()?;
    let mut best: Option<(usize, SyntaxKind, u32)> = None;
    for &(text, kind, p) in fixed_buckets().get(usize::from(first))? {
        if s.starts_with(text)
            && best.is_none_or(|(l, _, bp)| text.len() > l || (text.len() == l && p < bp))
        {
            best = Some((text.len(), kind, p));
        }
    }
    best
}

/// Lexes one non-trivia token in the `SPEC` state at the start of `s`.
fn lex_spec_token(s: &str) -> (usize, SyntaxKind, LexFlag) {
    let b = s.as_bytes();
    let mut best: Option<(usize, SyntaxKind, u32)> = None;
    let mut consider = |len: usize, kind: SyntaxKind, p: u32| {
        if len > 0 && best.is_none_or(|(l, _, bp)| len > l || (len == l && p < bp)) {
            best = Some((len, kind, p));
        }
    };
    consider(match_begin_module(b), BEGIN_MODULE, prio::BEGIN_MODULE);
    let dashes = run(b, |c| c == b'-');
    if dashes >= 4 {
        consider(dashes, SEPARATOR, prio::SEPARATOR);
    }
    let eqs = run(b, |c| c == b'=');
    if eqs >= 4 {
        consider(eqs, END_MODULE, prio::END_MODULE);
    }
    if let Some((l, k, p)) = match_fixed(s) {
        consider(l, k, p);
    }
    consider(match_number(b), NUMBER, prio::NUMBER);
    let mut string_error = None;
    if b.first() == Some(&b'"') {
        match match_string(b) {
            Ok(l) => consider(l, STRING, prio::STRING),
            Err(l) => string_error = Some(l.max(1)),
        }
    }
    consider(match_bad_junction(b), BAD_JUNCTION, prio::BAD_JUNCTION);
    consider(match_ident(s), IDENT, prio::IDENT);
    if let Some((l, k)) = match_step(b) {
        consider(l, k, prio::STEP);
    }
    match (best, string_error) {
        (Some((l, k, _)), _) => (l, k, LexFlag::Ok),
        (None, Some(l)) => (l, LEX_ERROR, LexFlag::BadString),
        (None, None) => {
            let l = s.chars().next().map_or(1, char::len_utf8);
            (l, LEX_ERROR, LexFlag::BadChar)
        }
    }
}

/// Length of a nested block comment starting with `(*`, and whether it closed.
fn block_comment_len(b: &[u8]) -> (usize, bool) {
    let mut depth = 1u32;
    let mut i = 2;
    while i < b.len() {
        if b[i] == b'(' && b.get(i + 1) == Some(&b'*') {
            depth += 1;
            i += 2;
        } else if b[i] == b'*' && b.get(i + 1) == Some(&b')') {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return (i, true);
            }
        } else {
            i += 1;
        }
    }
    (b.len(), false)
}

/// Builds tokens and tracks columns.
struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<LexToken>,
    cols: ColTracker,
}

impl Lexer<'_> {
    fn push(&mut self, kind: SyntaxKind, len: usize, flag: LexFlag) {
        let col = self.cols.column_at(self.src, self.pos);
        let start = u32::try_from(self.pos).unwrap_or(u32::MAX);
        let end = u32::try_from(self.pos + len).unwrap_or(u32::MAX);
        // Merge adjacent prelude text into one token.
        if kind == PRELUDE
            && let Some(last) = self.tokens.last_mut()
            && last.kind == PRELUDE
            && last.end == start
        {
            last.end = end;
            self.pos += len;
            return;
        }
        self.tokens.push(LexToken {
            kind,
            start,
            end,
            col,
            flag,
        });
        self.pos += len;
    }

    fn rest(&self) -> &str {
        self.src.get(self.pos..).unwrap_or("")
    }

    /// `DEFAULT` state. Returns true if a pragma was started.
    fn default_state(&mut self) -> bool {
        let b = self.src.as_bytes();
        let mut i = self.pos;
        while i < b.len() {
            if b[i] == b'-' {
                let m = match_begin_module(&b[i..]);
                if m > 0 || b[i..].starts_with(b"--->") {
                    if i > self.pos {
                        self.push(PRELUDE, i - self.pos, LexFlag::Ok);
                    }
                    if m > 0 {
                        self.push(BEGIN_MODULE, m, LexFlag::Ok);
                        return false;
                    }
                    self.push(PRAGMA_START, 4, LexFlag::Ok);
                    return true;
                }
            }
            i += 1;
        }
        if i > self.pos {
            self.push(PRELUDE, i - self.pos, LexFlag::Ok);
        }
        false
    }

    /// `PRAGMA` state, until a module header.
    fn pragma_state(&mut self) {
        while self.pos < self.src.len() {
            let s = self.rest();
            let b = s.as_bytes();
            let bm = match_begin_module(b);
            let num = run(b, is_digit);
            let ident = match_ident(s);
            let junk = match_bad_junction(b);
            let best = bm.max(num).max(ident).max(junk);
            if best == 0 {
                let l = s.chars().next().map_or(1, char::len_utf8);
                self.push(PRELUDE, l, LexFlag::Ok);
            } else if best == bm {
                self.push(BEGIN_MODULE, bm, LexFlag::Ok);
                return;
            } else if best == num {
                self.push(NUMBER, num, LexFlag::Ok);
            } else if best == junk {
                self.push(BAD_JUNCTION, junk, LexFlag::Ok);
            } else {
                self.push(IDENT, ident, LexFlag::Ok);
            }
        }
    }

    /// `SPEC` state, to the end of input.
    fn spec_state(&mut self) {
        while self.pos < self.src.len() {
            let s = self.rest();
            let b = s.as_bytes();
            let ws = run(b, |c| matches!(c, b' ' | b'\t' | b'\n' | b'\r'));
            if ws > 0 {
                self.push(WHITESPACE, ws, LexFlag::Ok);
            } else if b.starts_with(b"(*") {
                let (len, closed) = block_comment_len(b);
                let flag = if closed {
                    LexFlag::Ok
                } else {
                    LexFlag::UnterminatedComment
                };
                self.push(BLOCK_COMMENT, len, flag);
            } else if b.starts_with(b"\\*") {
                let len = run(b, |c| c != b'\n' && c != b'\r');
                self.push(LINE_COMMENT, len, LexFlag::Ok);
            } else {
                let (len, kind, flag) = lex_spec_token(s);
                self.push(kind, len, flag);
            }
        }
    }
}

/// Lexes a whole source file. The concatenated token texts equal `src`.
pub(crate) fn lex(src: &str) -> Vec<LexToken> {
    let mut lx = Lexer {
        src,
        pos: 0,
        tokens: Vec::with_capacity(src.len() / 3 + 4),
        cols: ColTracker::new(),
    };
    if lx.default_state() {
        lx.pragma_state();
    }
    lx.spec_state();
    lx.tokens
}

/// Lexes `src` entirely in the `SPEC` state, as a token manager created in
/// that state does (TLC's model-configuration reader does this).
pub(crate) fn lex_spec(src: &str) -> Vec<LexToken> {
    let mut lx = Lexer {
        src,
        pos: 0,
        tokens: Vec::with_capacity(src.len() / 3 + 4),
        cols: ColTracker::new(),
    };
    lx.spec_state();
    lx.tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(SyntaxKind, &str)> {
        lex(src)
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| (t.kind, &src[t.start as usize..t.end as usize]))
            .collect()
    }

    fn spec(body: &str) -> Vec<(SyntaxKind, String)> {
        let src = format!("---- MODULE M ----\n{body}");
        let toks = kinds(&src);
        toks[3..]
            .iter()
            .map(|(k, t)| (*k, (*t).to_string()))
            .collect()
    }

    fn one(body: &str) -> SyntaxKind {
        let t = spec(body);
        assert_eq!(t.len(), 1, "{body:?} lexed as {t:?}");
        t[0].0
    }

    #[test]
    fn lossless() {
        let src = "junk\n---- MODULE M ----\nx == 1 (* c (* nested *) *) \\* line\n====\ntrailing";
        let toks = lex(src);
        let joined: String = toks
            .iter()
            .map(|t| &src[t.start as usize..t.end as usize])
            .collect();
        assert_eq!(joined, src);
    }

    #[test]
    fn module_header_forms() {
        assert_eq!(kinds("x ---- MODULE")[0].0, BEGIN_MODULE);
        assert_eq!(kinds("-----MODULE")[0].0, BEGIN_MODULE);
        // A tab between the dashes and MODULE does not start a module.
        assert_eq!(kinds("----\tMODULE").len(), 0);
        assert_eq!(kinds("---> 1 a ---- MODULE")[0].0, PRAGMA_START);
    }

    #[test]
    fn keywords_and_identifiers() {
        assert_eq!(one("THEOREM"), THEOREM_KW);
        assert_eq!(one("THEOREMS"), IDENT);
        assert_eq!(one("LEMMA"), PROPOSITION_KW);
        assert_eq!(one("AXIOM"), ASSUMPTION_KW);
        assert_eq!(one("ENABLED"), PREFIX_OP);
        assert_eq!(one("_x"), IDENT);
        assert_eq!(one("1a"), IDENT);
        assert_eq!(one("@"), IDENT);
        assert_eq!(one("ℕ"), IDENT);
        assert_eq!(one("WF"), IDENT);
        assert_eq!(one("WFx"), IDENT);
        assert_eq!(one("WF_"), WF_KW);
        assert_eq!(
            spec("WF_vars"),
            vec![(WF_KW, "WF_".into()), (IDENT, "vars".into())]
        );
        assert_eq!(one("S_F"), IDENT);
    }

    #[test]
    fn numbers() {
        assert_eq!(one("123"), NUMBER);
        assert_eq!(one("\\b101"), NUMBER);
        assert_eq!(one("\\O17"), NUMBER);
        assert_eq!(one("\\hFF"), NUMBER);
        assert_eq!(one("\\o"), INFIX_OP);
        assert_eq!(
            spec("1.5"),
            vec![
                (NUMBER, "1".into()),
                (DOT, ".".into()),
                (NUMBER, "5".into())
            ]
        );
        assert_eq!(
            spec("1..5"),
            vec![
                (NUMBER, "1".into()),
                (INFIX_OP, "..".into()),
                (NUMBER, "5".into())
            ]
        );
    }

    #[test]
    fn strings() {
        assert_eq!(one(r#""a\"b\\c\n""#), STRING);
        assert_eq!(one(r#""\*""#), LEX_ERROR);
        assert_eq!(spec("\"abc\n")[0].0, LEX_ERROR);
    }

    #[test]
    fn operators_longest_match() {
        assert_eq!(one("-+->"), INFIX_OP);
        assert_eq!(one("-|-"), DEFBREAK_LIT);
        assert_eq!(one("(+)"), INFIX_OP);
        assert_eq!(one("[]"), BOX_OP);
        assert_eq!(one("[]ASSUME"), BOX_ASSUME_KW);
        assert_eq!(one("]_"), R_BRACK_UNDERSCORE);
        assert_eq!(one("\\in"), IN_OP);
        assert_eq!(one("\\intersect"), INFIX_OP);
        assert_eq!(one("\\E"), EXISTS_KW);
        assert_eq!(one("\\EE"), TEMPORAL_EXISTS_KW);
        assert_eq!(one("≜"), DEF_EQ);
        assert_eq!(one("x./\\"), BAD_JUNCTION);
        assert_eq!(
            spec("==="),
            vec![(DEF_EQ, "==".into()), (EQ_OP, "=".into())]
        );
        assert_eq!(one("===="), END_MODULE);
        assert_eq!(one("-----"), SEPARATOR);
    }

    #[test]
    fn steps() {
        assert_eq!(one("<1>2"), STEP_NUMBER);
        assert_eq!(one("<1>2."), STEP_DOT);
        assert_eq!(one("<1>"), STEP_BARE);
        assert_eq!(one("<*>"), STEP_BARE);
        assert_eq!(one("<+>a"), STEP_IMPLICIT);
        assert_eq!(one("<1>."), STEP_UNNUMBERED);
        assert_eq!(one("<1>a_b"), STEP_NUMBER);
        assert_eq!(
            spec("<4"),
            vec![(INFIX_OP, "<".into()), (NUMBER, "4".into())]
        );
    }

    #[test]
    fn comments() {
        assert_eq!(spec("(* a (* b *) c *) x").len(), 1);
        assert_eq!(spec("\\* x\ny").len(), 1);
        let toks = lex("---- MODULE M ----\n(* open");
        assert_eq!(
            toks.last().map(|t| t.flag),
            Some(LexFlag::UnterminatedComment)
        );
    }

    #[test]
    fn columns_follow_javacc() {
        let src = "---- MODULE M ----\n\t/\\ x\n  𝔸 ∧";
        let toks = lex(src);
        let cols: Vec<(String, u32)> = toks
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| (src[t.start as usize..t.end as usize].to_string(), t.col))
            .collect();
        // A tab advances to column 9; an astral char takes two columns.
        assert!(cols.contains(&("/\\".to_string(), 9)));
        assert!(cols.contains(&("x".to_string(), 12)));
        assert!(cols.contains(&("∧".to_string(), 6)));
    }
}
