//! The cfg reader: a port of the semantics of TLC's `ModelConfig.parse` and
//! `ModelConfig.parseValue` at upstream commit `94d0c50`.
//!
//! TLC pulls tokens one at a time from SANY's token manager and fails at the
//! first problem, so this does too: errors are produced in the same order,
//! and a lexical error counts only once the reader asks for that token.
//! Keywords, punctuation and `TRUE`/`FALSE` are recognised by their exact
//! text (TLC compares token images), so `←` is not `<-`.

use crate::diagnostics::{Code, Diagnostic, TlcError};
use crate::model::{ConstantEntry, ModelConfig, Name};
use crate::value::{ConfigValue, Spanned};
use dive_syntax::tokens::{LexError, Token, tokenize_spec};
use dive_syntax::{SyntaxKind, TextRange, TextSize};
use std::collections::BTreeSet;

/// Sets nested deeper than this are refused with `DIVE-C0030`. TLC parses
/// them recursively and fails with a stack overflow somewhere deeper,
/// depending on `-Xss`; dive refuses explicitly instead (cf. ADR 0009).
pub const MAX_SET_NESTING: usize = 1024;

/// Every keyword TLC's reader knows (`ModelConfig.ALL_KEYWORDS`). A token
/// whose text is one of these ends a list or constant section.
pub const KEYWORDS: [&str; 22] = [
    "CONSTANT",
    "CONSTANTS",
    "CONSTRAINT",
    "CONSTRAINTS",
    "ACTION_CONSTRAINT",
    "ACTION_CONSTRAINTS",
    "INVARIANT",
    "INVARIANTS",
    "INIT",
    "NEXT",
    "VIEW",
    "SYMMETRY",
    "SPECIFICATION",
    "PROPERTY",
    "PROPERTIES",
    "ALIAS",
    "POSTCONDITION",
    "POSTCONDITIONS",
    "_PERIODIC",
    "_RL_REWARD",
    "_POSSIBLE",
    "CHECK_DEADLOCK",
];

/// Whether TLC's reader treats `text` as a section keyword.
#[must_use]
pub fn is_keyword(text: &str) -> bool {
    KEYWORDS.contains(&text)
}

/// One token as the reader sees it.
#[derive(Clone, Copy)]
struct Tok<'a> {
    text: &'a str,
    kind: SyntaxKind,
    range: TextRange,
    eof: bool,
}

/// A pull-based token stream, as TLC's `getNextToken`.
struct Cursor<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(src: &'a str) -> Self {
        let tokens = tokenize_spec(src)
            .into_iter()
            .filter(|t| t.error.is_some() || !t.kind.is_trivia())
            .collect();
        Self {
            src,
            tokens,
            pos: 0,
        }
    }

    fn eof_offset(&self) -> TextSize {
        TextSize::try_from(self.src.len()).unwrap_or(TextSize::from(u32::MAX))
    }

    /// The next token. At end of input this keeps returning end of file,
    /// as the `JavaCC` token manager does.
    fn next(&mut self) -> Result<Tok<'a>, Diagnostic> {
        let Some(t) = self.tokens.get(self.pos).copied() else {
            return Ok(Tok {
                text: "",
                kind: SyntaxKind::LEX_ERROR,
                range: TextRange::empty(self.eof_offset()),
                eof: true,
            });
        };
        self.pos += 1;
        if let Some(e) = t.error {
            return Err(self.lex_error(t, e));
        }
        Ok(Tok {
            text: self
                .src
                .get(std::ops::Range::<usize>::from(t.range))
                .unwrap_or(""),
            kind: t.kind,
            range: t.range,
            eof: false,
        })
    }

    fn lex_error(&self, t: Token, e: LexError) -> Diagnostic {
        let text = self
            .src
            .get(std::ops::Range::<usize>::from(t.range))
            .unwrap_or("");
        let (code, message, help, line) = match e {
            LexError::BadString => (
                Code::BadString,
                "string literal with a bad escape or no closing quote on its line".to_owned(),
                "close the string on the same line; the only escapes are \\n \\t \\r \\f \\\\ \\\""
                    .to_owned(),
                javacc_line(self.src, t.range.start()),
            ),
            LexError::UnterminatedComment => (
                Code::UnterminatedComment,
                "block comment `(*` is never closed".to_owned(),
                "close it with `*)`".to_owned(),
                // JavaCC reports end of file after the appended newline.
                eof_line(self.src).saturating_add(1),
            ),
            _ => {
                let c = text.chars().next().unwrap_or('\u{fffd}');
                let help = if c == '\u{feff}' {
                    "remove the byte-order mark at the start of the file; TLC does not skip it"
                        .to_owned()
                } else {
                    "TLA+ cfg files use the TLA+ token set; remove or quote this character"
                        .to_owned()
                };
                (
                    Code::UnexpectedCharacter,
                    format!("unexpected character {c:?} (U+{:04X})", u32::from(c)),
                    help,
                    javacc_line(self.src, t.range.start()),
                )
            }
        };
        Diagnostic::error(
            code,
            t.range,
            message,
            Some(TlcError::cfg(5007, line, None)),
        )
        .with_help(help)
    }

    /// The line TLC's `scs.getBeginLine()` gives once `tok` was read: the
    /// token's first line, or at end of file the line of the newline TLC
    /// appends to the input.
    fn line(&self, tok: &Tok<'_>) -> u32 {
        if tok.eof {
            eof_line(self.src)
        } else {
            javacc_line(self.src, tok.range.start())
        }
    }
}

/// 1-based line of `offset`, counting line terminators as `JavaCC`'s
/// `SimpleCharStream` does: `\n`, `\r\n` and a lone `\r` each end a line.
pub(crate) fn javacc_line(src: &str, offset: TextSize) -> u32 {
    let end = usize::from(offset).min(src.len());
    let b = src.as_bytes();
    let mut line = 1u32;
    for i in 0..end {
        match b[i] {
            b'\n' => line = line.saturating_add(1),
            b'\r' if b.get(i + 1) != Some(&b'\n') => line = line.saturating_add(1),
            _ => {}
        }
    }
    line
}

/// The line of the `\n` TLC appends to every cfg before reading it. A file
/// ending in `\r` gets no new line, since `\r\n` is one terminator.
fn eof_line(src: &str) -> u32 {
    let line = javacc_line(
        src,
        TextSize::try_from(src.len()).unwrap_or(TextSize::from(u32::MAX)),
    );
    if src.ends_with('\r') {
        line.saturating_sub(1).max(1)
    } else {
        line
    }
}

fn cfg_error(
    code: Code,
    tlc: (u16, u32, Option<&'static str>),
    range: TextRange,
    message: impl Into<String>,
) -> Diagnostic {
    let (tlc_code, line, detail) = tlc;
    Diagnostic::error(
        code,
        range,
        message,
        Some(TlcError::cfg(tlc_code, line, detail)),
    )
}

/// The static spelling of a keyword, for [`TlcError::detail`].
fn static_keyword(text: &str) -> Option<&'static str> {
    KEYWORDS.iter().copied().find(|k| *k == text)
}

/// `CFG_TWICE_KEYWORD`, reported at the second occurrence.
fn duplicate(keyword: &Tok<'_>, loc: u32) -> Diagnostic {
    cfg_error(
        Code::DuplicateKeyword,
        (5004, loc, static_keyword(keyword.text)),
        keyword.range,
        format!("`{}` appears twice", keyword.text),
    )
    .with_help(format!("keep one `{}` line", keyword.text))
}

/// `CFG_MISSING_ID`: a keyword at end of file.
fn missing_name(keyword: &Tok<'_>, loc: u32, message: String, help: String) -> Diagnostic {
    cfg_error(
        Code::MissingName,
        (5003, loc, static_keyword(keyword.text)),
        keyword.range,
        message,
    )
    .with_help(help)
}

/// How a TLC "expected" parameter reads in a dive message.
fn shown(what: &str) -> String {
    match what {
        "a keyword" => "a cfg keyword".to_owned(),
        "= or <-" => "`=` or `<-`".to_owned(),
        "a value" | "TRUE or FALSE" => what.to_owned(),
        _ => format!("`{what}`"),
    }
}

fn name_of(t: &Tok<'_>) -> Name {
    Name {
        text: t.text.to_owned(),
        span: t.range,
    }
}

struct Reader<'a> {
    cur: Cursor<'a>,
    cfg: ModelConfig,
    seen_model_values: BTreeSet<String>,
    warnings: Vec<Diagnostic>,
}

/// Which single-valued keyword a section sets.
#[derive(Clone, Copy)]
enum Single {
    Init,
    Next,
    Specification,
    View,
    Symmetry,
    Alias,
    Periodic,
    RlReward,
}

/// Which list a list section appends to.
#[derive(Clone, Copy)]
enum List {
    Invariants,
    Properties,
    Constraints,
    ActionConstraints,
    Postconditions,
    Possible,
}

impl<'a> Reader<'a> {
    fn single(&mut self, s: Single) -> &mut Option<Name> {
        match s {
            Single::Init => &mut self.cfg.init,
            Single::Next => &mut self.cfg.next,
            Single::Specification => &mut self.cfg.specification,
            Single::View => &mut self.cfg.view,
            Single::Symmetry => &mut self.cfg.symmetry,
            Single::Alias => &mut self.cfg.alias,
            Single::Periodic => &mut self.cfg.periodic,
            Single::RlReward => &mut self.cfg.rl_reward,
        }
    }

    fn list(&mut self, l: List) -> &mut Vec<Name> {
        match l {
            List::Invariants => &mut self.cfg.invariants,
            List::Properties => &mut self.cfg.properties,
            List::Constraints => &mut self.cfg.constraints,
            List::ActionConstraints => &mut self.cfg.action_constraints,
            List::Postconditions => &mut self.cfg.postconditions,
            List::Possible => &mut self.cfg.possible,
        }
    }

    /// `CFG_EXPECTED_SYMBOL` at the current token; `what` is TLC's text.
    fn expected(&self, code: Code, tok: &Tok<'_>, what: &'static str) -> Diagnostic {
        let found = if tok.eof {
            "end of file".to_owned()
        } else {
            format!("`{}`", tok.text)
        };
        cfg_error(
            code,
            (5006, self.cur.line(tok), Some(what)),
            tok.range,
            format!("expected {}, found {found}", shown(what)),
        )
    }

    /// Warns when a name position holds something TLC can never resolve.
    fn check_name(&mut self, tok: &Tok<'_>, role: &str) {
        if is_keyword(tok.text) {
            self.warnings.push(
                Diagnostic::warning(
                    Code::KeywordAsName,
                    tok.range,
                    format!(
                        "the keyword `{}` is read as the {role}; TLC will look for a definition named `{}`",
                        tok.text, tok.text
                    ),
                )
                .with_help(format!("give the {role} before the next section keyword")),
            );
        } else if tok.kind != SyntaxKind::IDENT {
            let help = if tok.text == "," {
                "cfg names are separated by whitespace, not commas".to_owned()
            } else {
                "a cfg name must be an identifier defined in the specification".to_owned()
            };
            self.warnings.push(
                Diagnostic::warning(
                    Code::NotAnIdentifier,
                    tok.range,
                    format!(
                        "`{}` is not an identifier, but TLC reads it as the {role} and will fail to find its definition",
                        tok.text
                    ),
                )
                .with_help(help),
            );
        }
    }

    /// The main loop of `ModelConfig.parse`.
    fn run(&mut self) -> Result<(), Diagnostic> {
        let mut tt = self.cur.next()?;
        while !tt.eof {
            let keyword = tt;
            // TLC's `loc`: the section keyword's line, used by the errors
            // that name the keyword and by `CFG_GENERAL`.
            let loc = self.cur.line(&keyword);
            tt = match keyword.text {
                "INIT" => self.single_section(&keyword, loc, Single::Init)?,
                "NEXT" => self.single_section(&keyword, loc, Single::Next)?,
                "SPECIFICATION" => self.single_section(&keyword, loc, Single::Specification)?,
                "VIEW" => self.single_section(&keyword, loc, Single::View)?,
                "SYMMETRY" => self.single_section(&keyword, loc, Single::Symmetry)?,
                "ALIAS" => self.single_section(&keyword, loc, Single::Alias)?,
                "_PERIODIC" => self.single_section(&keyword, loc, Single::Periodic)?,
                "_RL_REWARD" => self.single_section(&keyword, loc, Single::RlReward)?,
                "INVARIANT" | "INVARIANTS" => self.list_section(&keyword, List::Invariants)?,
                "PROPERTY" | "PROPERTIES" => self.list_section(&keyword, List::Properties)?,
                "CONSTRAINT" | "CONSTRAINTS" => self.list_section(&keyword, List::Constraints)?,
                "ACTION_CONSTRAINT" | "ACTION_CONSTRAINTS" => {
                    self.list_section(&keyword, List::ActionConstraints)?
                }
                "POSTCONDITION" | "POSTCONDITIONS" => {
                    self.list_section(&keyword, List::Postconditions)?
                }
                "_POSSIBLE" => self.list_section(&keyword, List::Possible)?,
                "CONSTANT" | "CONSTANTS" => self.constant_section(loc)?,
                "CHECK_DEADLOCK" => self.deadlock_section(&keyword, loc)?,
                _ => return Err(self.not_a_keyword(&keyword)),
            };
        }
        Ok(())
    }

    fn not_a_keyword(&self, keyword: &Tok<'_>) -> Diagnostic {
        let d = self.expected(Code::ExpectedKeyword, keyword, "a keyword");
        let upper = keyword.text.to_ascii_uppercase();
        if is_keyword(&upper) {
            d.with_help(format!("keywords are case sensitive: write `{upper}`"))
        } else {
            let listed: Vec<&str> = KEYWORDS
                .iter()
                .filter(|k| !k.starts_with('_'))
                .copied()
                .collect();
            d.with_help(format!("sections start with one of {}", listed.join(", ")))
        }
    }

    /// `INIT`, `NEXT`, ...: exactly one following token, once per file.
    fn single_section(
        &mut self,
        keyword: &Tok<'a>,
        loc: u32,
        s: Single,
    ) -> Result<Tok<'a>, Diagnostic> {
        let tt = self.cur.next()?;
        if tt.eof {
            return Err(missing_name(
                keyword,
                loc,
                format!("`{}` is not followed by a name", keyword.text),
                format!("write `{} <name>`", keyword.text),
            ));
        }
        if self.single(s).is_some() {
            return Err(duplicate(keyword, loc));
        }
        self.check_name(&tt, &format!("{} name", keyword.text));
        *self.single(s) = Some(name_of(&tt));
        self.cur.next()
    }

    /// `INVARIANT`, `PROPERTY`, ...: every token up to the next keyword.
    fn list_section(&mut self, keyword: &Tok<'a>, l: List) -> Result<Tok<'a>, Diagnostic> {
        loop {
            let tt = self.cur.next()?;
            if tt.eof || is_keyword(tt.text) {
                return Ok(tt);
            }
            self.check_name(&tt, &format!("{} name", keyword.text));
            let n = name_of(&tt);
            self.list(l).push(n);
        }
    }

    /// `CONSTANT`/`CONSTANTS`: entries up to the next keyword.
    fn constant_section(&mut self, loc: u32) -> Result<Tok<'a>, Diagnostic> {
        loop {
            let tt = self.cur.next()?;
            if tt.eof || is_keyword(tt.text) {
                return Ok(tt);
            }
            self.constant(tt, loc)?;
        }
    }

    /// `CHECK_DEADLOCK TRUE|FALSE`, once per file.
    fn deadlock_section(&mut self, keyword: &Tok<'a>, loc: u32) -> Result<Tok<'a>, Diagnostic> {
        let tt = self.cur.next()?;
        if tt.eof {
            return Err(missing_name(
                keyword,
                loc,
                "`CHECK_DEADLOCK` is not followed by TRUE or FALSE".to_owned(),
                "write `CHECK_DEADLOCK TRUE` or `CHECK_DEADLOCK FALSE`".to_owned(),
            ));
        }
        let value = match tt.text {
            "TRUE" => true,
            "FALSE" => false,
            _ => {
                return Err(self
                    .expected(Code::ExpectedBoolean, &tt, "TRUE or FALSE")
                    .with_help("the value is case sensitive: `TRUE` or `FALSE`"));
            }
        };
        if self.cfg.check_deadlock.is_some() {
            return Err(duplicate(keyword, loc));
        }
        self.cfg.check_deadlock = Some(Spanned {
            value,
            span: tt.range,
        });
        self.cur.next()
    }

    /// One entry of a constant section; `first` is its first token and
    /// `loc` the line of the section keyword.
    fn constant(&mut self, first: Tok<'a>, loc: u32) -> Result<(), Diagnostic> {
        self.check_name(&first, "constant name");
        let mut lhs = first.text.to_owned();
        let mut span = first.range;
        let mut tt = self.cur.next()?;
        // `frob!bar!glitch` names a constant of an instantiated module.
        while tt.text == "!" {
            tt = self.cur.next()?;
            lhs.push('!');
            lhs.push_str(tt.text);
            if !tt.eof {
                span = span.cover(tt.range);
            }
            tt = self.cur.next()?;
        }
        let name = Name { text: lhs, span };
        if tt.text == "<-" {
            self.substitution(name)
        } else {
            self.assignment(name, tt, loc)
        }
    }

    /// After `C <-`: `D` or `[M] D`.
    fn substitution(&mut self, name: Name) -> Result<(), Diagnostic> {
        let mut tt = self.cur.next()?;
        let mut module = None;
        if tt.text == "[" {
            tt = self.cur.next()?;
            if tt.eof {
                return Err(self.missing_identifier(&tt, "<-["));
            }
            module = Some(name_of(&tt));
            tt = self.cur.next()?;
            if tt.text != "]" {
                return Err(self.expected(Code::UnclosedModule, &tt, "]"));
            }
            tt = self.cur.next()?;
            if tt.eof {
                return Err(self.missing_identifier(&tt, "<-[mod]"));
            }
        } else if tt.eof {
            return Err(self.missing_identifier(&tt, "<-"));
        }
        self.check_name(&tt, "substitution target");
        self.cfg.constants.push(ConstantEntry::Substitution {
            name,
            module,
            target: name_of(&tt),
        });
        Ok(())
    }

    /// After `C`, at `tt`: optional `(args)`, then `= v` or `= [M] v`.
    fn assignment(&mut self, name: Name, mut tt: Tok<'a>, loc: u32) -> Result<(), Diagnostic> {
        let mut args = Vec::new();
        if tt.text == "(" {
            let open = tt;
            loop {
                tt = self.cur.next()?;
                args.push(self.value(tt)?);
                tt = self.cur.next()?;
                if tt.text != "," {
                    break;
                }
            }
            if tt.text != ")" {
                let range = if tt.eof { open.range } else { tt.range };
                return Err(cfg_error(
                    Code::UnclosedArguments,
                    (5002, loc, None),
                    range,
                    "constant operator arguments are not closed by `)`",
                )
                .with_help("write arguments as `F(v1, v2) = v`; `F()` is not allowed"));
            }
            tt = self.cur.next()?;
        }
        if tt.text != "=" {
            let d = self.expected(Code::ExpectedAssignment, &tt, "= or <-");
            return Err(self.assignment_help(d, &tt));
        }
        tt = self.cur.next()?;
        let mut module = None;
        if tt.text == "[" {
            tt = self.cur.next()?;
            if tt.eof {
                return Err(self.missing_identifier(&tt, "=["));
            }
            module = Some(name_of(&tt));
            tt = self.cur.next()?;
            if tt.text != "]" {
                return Err(self.expected(Code::UnclosedModule, &tt, "]"));
            }
            tt = self.cur.next()?;
        }
        let value = self.value(tt)?;
        self.cfg.constants.push(ConstantEntry::Assignment {
            name,
            args,
            module,
            value,
        });
        Ok(())
    }

    /// Explains the common ways to reach `expected = or <-`.
    fn assignment_help(&self, d: Diagnostic, tt: &Tok<'_>) -> Diagnostic {
        let previous_value = self.cfg.constants.last().and_then(|e| match e {
            ConstantEntry::Assignment { value, .. } => match &value.value {
                ConfigValue::ModelValue(m) => Some(m.as_str()),
                _ => None,
            },
            ConstantEntry::Substitution { .. } => None,
        });
        match (previous_value, tt.text) {
            (Some("-"), _) => d.with_help(
                "cfg values cannot be negative: TLC read `-` as a model value; define the value in the spec and substitute it with `<-`",
            ),
            (Some("<<" | "⟨"), _) => d.with_help(
                "cfg values cannot be tuples: TLC read `<<` as a model value; define the value in the spec and substitute it with `<-`",
            ),
            (_, "←") => d.with_help("write the substitution arrow as `<-`; TLC does not accept `←`"),
            _ => d.with_help("constant entries are `C = value` or `C <- Definition`"),
        }
    }

    fn missing_identifier(&self, tok: &Tok<'_>, after: &'static str) -> Diagnostic {
        cfg_error(
            Code::MissingIdentifier,
            (5005, self.cur.line(tok), Some(after)),
            tok.range,
            format!("expected an identifier after `{after}`, found end of file"),
        )
    }

    fn note_model_value(&mut self, tok: &Tok<'_>) {
        if !self.seen_model_values.contains(tok.text) {
            self.seen_model_values.insert(tok.text.to_owned());
            self.cfg.model_values.push(tok.text.to_owned());
        }
        if is_keyword(tok.text) {
            self.warnings.push(
                Diagnostic::warning(
                    Code::KeywordAsName,
                    tok.range,
                    format!("the keyword `{}` is read as a model value", tok.text),
                )
                .with_help("give the value before the next section keyword"),
            );
        } else if tok.kind != SyntaxKind::IDENT {
            self.warnings.push(
                Diagnostic::warning(
                    Code::OddModelValue,
                    tok.range,
                    format!(
                        "`{}` is not an identifier, but TLC reads it as a model value",
                        tok.text
                    ),
                )
                .with_help("cfg values are integers, strings, TRUE, FALSE, sets `{...}` and model-value identifiers"),
            );
        }
    }

    /// A value that is not a set (TLC's `parseValue` without the `{` case).
    fn scalar(&mut self, tok: &Tok<'_>) -> Result<Spanned<ConfigValue>, Diagnostic> {
        let value = if tok.kind == SyntaxKind::NUMBER {
            // TLC: Integer.parseInt(image). Only plain decimal digits in
            // 32-bit range parse; anything else throws.
            match tok.text.parse::<i32>() {
                Ok(v) if tok.text.bytes().all(|b| b.is_ascii_digit()) => ConfigValue::Int(v),
                _ => {
                    let help = if tok.text.starts_with('\\') {
                        "TLC reads cfg integers in decimal only; write the value in decimal"
                    } else {
                        "TLC cfg integers are 32-bit (at most 2147483647); define larger values in the spec and substitute them with `<-`"
                    };
                    return Err(Diagnostic::error(
                        Code::BadInteger,
                        tok.range,
                        format!("TLC cannot read the integer `{}`", tok.text),
                        Some(TlcError::general("java.lang.NumberFormatException")),
                    )
                    .with_help(help));
                }
            }
        } else if tok.kind == SyntaxKind::STRING {
            let inner = tok
                .text
                .get(1..tok.text.len().saturating_sub(1))
                .unwrap_or("");
            ConfigValue::String(inner.to_owned())
        } else if tok.text == "TRUE" {
            ConfigValue::Bool(true)
        } else if tok.text == "FALSE" {
            ConfigValue::Bool(false)
        } else if !tok.eof {
            self.note_model_value(tok);
            ConfigValue::ModelValue(tok.text.to_owned())
        } else {
            return Err(self
                .expected(Code::MissingValue, tok, "a value")
                .with_help("the file ends where a constant's value should be"));
        };
        Ok(Spanned {
            value,
            span: tok.range,
        })
    }

    /// TLC's `parseValue`, starting at `first`. Iterative, so deep nesting
    /// cannot overflow the stack.
    fn value(&mut self, first: Tok<'a>) -> Result<Spanned<ConfigValue>, Diagnostic> {
        let mut open: Vec<(TextRange, Vec<Spanned<ConfigValue>>)> = Vec::new();
        let mut tok = first;
        'value: loop {
            let is_set =
                tok.text == "{" && tok.kind != SyntaxKind::NUMBER && tok.kind != SyntaxKind::STRING;
            let mut done = if is_set {
                if open.len() >= MAX_SET_NESTING {
                    return Err(Diagnostic::error(
                        Code::NestingTooDeep,
                        tok.range,
                        format!("sets nested more than {MAX_SET_NESTING} deep are not supported"),
                        None,
                    ));
                }
                let next = self.cur.next()?;
                if next.text == "}" {
                    Spanned {
                        value: ConfigValue::Set(Vec::new()),
                        span: tok.range.cover(next.range),
                    }
                } else {
                    open.push((tok.range, Vec::new()));
                    tok = next;
                    continue 'value;
                }
            } else {
                self.scalar(&tok)?
            };
            // Hand the finished value to the innermost open set.
            loop {
                let Some((_, elems)) = open.last_mut() else {
                    return Ok(done);
                };
                elems.push(done);
                let t = self.cur.next()?;
                if t.text == "," {
                    tok = self.cur.next()?;
                    continue 'value;
                }
                if t.text != "}" {
                    return Err(self
                        .expected(Code::UnclosedSet, &t, "}")
                        .with_help("separate set elements with `,` and close the set with `}`"));
                }
                let Some((start, elems)) = open.pop() else {
                    return Ok(Spanned {
                        value: ConfigValue::Set(Vec::new()),
                        span: t.range,
                    });
                };
                done = Spanned {
                    value: ConfigValue::Set(elems),
                    span: start.cover(t.range),
                };
            }
        }
    }
}

/// Reads a whole cfg text. Returns the configuration, or the first error,
/// plus warnings found before it.
pub(crate) fn read(src: &str) -> (Option<ModelConfig>, Vec<Diagnostic>) {
    let mut r = Reader {
        cur: Cursor::new(src),
        cfg: ModelConfig::default(),
        seen_model_values: BTreeSet::new(),
        warnings: Vec::new(),
    };
    match r.run() {
        Ok(()) => (Some(r.cfg), r.warnings),
        Err(e) => {
            let mut d = r.warnings;
            d.push(e);
            (None, d)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_follow_javacc() {
        let s = "a\nb\r\nc\rd";
        assert_eq!(javacc_line(s, TextSize::from(0)), 1);
        assert_eq!(javacc_line(s, TextSize::from(2)), 2);
        assert_eq!(javacc_line(s, TextSize::from(5)), 3);
        assert_eq!(javacc_line(s, TextSize::from(7)), 4);
        assert_eq!(eof_line("x\n"), 2);
        assert_eq!(eof_line("x"), 1);
        assert_eq!(eof_line("x\r"), 1);
        assert_eq!(eof_line("x\r\n"), 2);
        assert_eq!(eof_line(""), 1);
    }
}
