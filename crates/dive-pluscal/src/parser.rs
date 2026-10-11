//! Recursive descent for original C/P source grammar; bounded statement arena.
use crate::{
    Code, Diagnostic, Parsed,
    ast::{
        Algorithm, Assignment, Binding, BindingKind, Block, Body, Branch, Decl, Expr, Fairness,
        Label, Macro, Name, Procedure, Process, Span, Statement, StatementKind, StmtId, Syntax,
        Token, WithBinding,
    },
    lexer,
};
use serde::{Deserialize, Serialize};

/// Resource settings. Hard byte/depth ceilings also prevent unbounded allocation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Options {
    /// Maximum complete source bytes (hard ceiling: 4 MiB).
    pub max_bytes: usize,
    /// Maximum significant tokens (also bounded by bytes).
    pub max_tokens: usize,
    /// Maximum statement/declaration/branch nodes.
    pub max_nodes: usize,
    /// Maximum grammar and expression nesting (hard ceiling: 256).
    pub max_depth: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            max_bytes: 1_048_576,
            max_tokens: 100_000,
            max_nodes: 50_000,
            max_depth: 128,
        }
    }
}

/// Parses a complete standalone algorithm source, rejecting trailing tokens.
/// This certifies source grammar only; translation remains explicitly pending.
///
/// # Errors
/// A lexical, grammar or resource diagnostic with an original source span.
pub fn parse(source: &str, opts: &Options) -> Result<Parsed, Diagnostic> {
    parse_at(source, 0..source.len(), opts)
}

/// Parses an explicitly selected algorithm region in a surrounding source file.
/// The region must include the `--algorithm`/`--fair` marker and exclude enclosing
/// comment delimiters and unrelated TLA+ text. Every selected token is consumed.
/// Returned spans refer to the complete original source, not the selected slice.
/// Parsing runs on a scoped worker with a 16 MiB stack, so the hard recursive
/// depth ceiling does not depend on the caller's stack size. The worker is
/// joined before returning and never outlives the borrowed input.
///
/// # Errors
/// Invalid region boundaries or lexical/grammar/resource diagnostics.
pub fn parse_at(
    source: &str,
    region: std::ops::Range<usize>,
    opts: &Options,
) -> Result<Parsed, Diagnostic> {
    parse_scoped(source, region, opts, true)
}

pub(crate) fn parse_prefix_at(
    source: &str,
    region: std::ops::Range<usize>,
    opts: &Options,
) -> Result<Parsed, Diagnostic> {
    parse_scoped(source, region, opts, false)
}

fn parse_scoped(
    source: &str,
    region: std::ops::Range<usize>,
    opts: &Options,
    complete: bool,
) -> Result<Parsed, Diagnostic> {
    std::thread::scope(|scope| {
        let failure = |code, message: String| Diagnostic {
            code,
            span: Span::default(),
            message,
        };
        let worker = std::thread::Builder::new()
            .name("dive-pluscal-parser".into())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, || parse_selected(source, region, opts, complete))
            .map_err(|error| {
                failure(
                    Code::Resource,
                    format!("cannot start PlusCal parser: {error}"),
                )
            })?;
        worker.join().map_err(|_| {
            failure(
                Code::Internal,
                "PlusCal parser worker failed; report this input as a bug".into(),
            )
        })?
    })
}

fn parse_selected(
    source: &str,
    region: std::ops::Range<usize>,
    opts: &Options,
    complete: bool,
) -> Result<Parsed, Diagnostic> {
    let nearest = |offset: usize| {
        let mut offset = offset.min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    let start = nearest(region.start);
    let span = Span {
        start,
        end: nearest(region.end).max(start),
    };
    let error = |code, message: &str| Diagnostic {
        code,
        span,
        message: message.into(),
    };
    if source.len() > opts.max_bytes.min(4_194_304) {
        return Err(error(Code::Resource, "PlusCal source byte limit reached"));
    }
    let Some(text) = source.get(region.clone()) else {
        return Err(error(Code::Syntax, "invalid PlusCal source region"));
    };
    let trimmed = text.trim_start();
    let begin = region.start + text.len() - trimmed.len();
    let (prefix, fair) = if trimmed.starts_with("--algorithm") {
        ("--algorithm", false)
    } else if trimmed.starts_with("--fair") {
        ("--fair", true)
    } else {
        return Err(error(
            Code::Syntax,
            "expected --algorithm or --fair algorithm",
        ));
    };
    let after = begin + prefix.len();
    if source
        .get(after..region.end)
        .and_then(|s| s.chars().next())
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(error(
            Code::Syntax,
            "algorithm marker requires a token boundary",
        ));
    }
    let (tokens, deferred) = if complete {
        (
            lexer::tokenize(source, after, region.end, opts.max_tokens)?,
            Vec::new(),
        )
    } else {
        lexer::tokenize_prefix(source, after, region.end, opts.max_tokens)?
    };
    let mut parser = Parser {
        tokens,
        pos: 0,
        statements: Vec::new(),
        syntax: Syntax::P,
        opts,
        nodes: 0,
        in_macro: false,
        end: region.end,
    };
    if fair {
        parser.need("algorithm")?;
    }
    let algorithm = parser
        .algorithm(begin, fair)
        .map_err(|error| deferred.first().cloned().unwrap_or(error))?;
    if let Some(error) = deferred
        .into_iter()
        .find(|error| error.span.start < algorithm.span.end)
    {
        return Err(error);
    }
    if complete && parser.pos != parser.tokens.len() {
        return Err(parser.error(Code::Syntax, "unexpected trailing algorithm content"));
    }
    parser.tokens.truncate(parser.pos);
    Ok(Parsed {
        source: source.to_owned(),
        algorithm,
        statements: parser.statements,
        tokens: parser.tokens,
    })
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    statements: Vec<Statement>,
    syntax: Syntax,
    opts: &'a Options,
    nodes: usize,
    in_macro: bool,
    end: usize,
}
impl Parser<'_> {
    fn peek(&self) -> &str {
        self.tokens.get(self.pos).map_or("", |t| t.text.as_str())
    }
    fn at(&self, n: usize) -> &str {
        self.tokens
            .get(self.pos.saturating_add(n))
            .map_or("", |t| t.text.as_str())
    }
    fn start(&self) -> usize {
        self.tokens.get(self.pos).map_or(self.end, |t| t.span.start)
    }
    fn last(&self) -> usize {
        self.pos
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .map_or(self.start(), |t| t.span.end)
    }
    fn error(&self, code: Code, message: &str) -> Diagnostic {
        Diagnostic {
            code,
            span: self.tokens.get(self.pos).map_or(
                Span {
                    start: self.end,
                    end: self.end,
                },
                |t| t.span,
            ),
            message: message.into(),
        }
    }
    fn take(&mut self) -> Result<Token, Diagnostic> {
        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| self.error(Code::Syntax, "unexpected end of algorithm"))?;
        self.pos += 1;
        Ok(token)
    }
    fn eat(&mut self, word: &str) -> bool {
        if self.peek() == word {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn need(&mut self, word: &str) -> Result<(), Diagnostic> {
        if self.eat(word) {
            Ok(())
        } else {
            Err(self.error(
                Code::Syntax,
                &format!("expected {word:?}, found {:?}", self.peek()),
            ))
        }
    }
    fn charge(&mut self, depth: usize) -> Result<(), Diagnostic> {
        if depth > self.opts.max_depth.min(256) || self.nodes >= self.opts.max_nodes {
            return Err(self.error(Code::Resource, "PlusCal node/nesting limit reached"));
        }
        self.nodes += 1;
        Ok(())
    }
    fn name(&mut self) -> Result<Name, Diagnostic> {
        let token = self.take()?;
        // The Java grammar reads names as algorithm tokens. Preserve its
        // spelling rather than silently applying SANY name-resolution rules.
        if token.text.is_empty()
            || matches!(
                token.text.as_str(),
                ";" | "," | "(" | ")" | "{" | "}" | ":=" | "≔"
            )
        {
            return Err(Diagnostic {
                code: Code::Syntax,
                span: token.span,
                message: "expected a name token".into(),
            });
        }
        Ok(Name {
            text: token.text,
            span: token.span,
        })
    }
    fn semi(&mut self) -> Result<(), Diagnostic> {
        if self.eat(";")
            || matches!(
                self.peek(),
                "end"
                    | "begin"
                    | "else"
                    | "elsif"
                    | "or"
                    | "do"
                    | "macro"
                    | "procedure"
                    | "process"
                    | "fair"
                    | "define"
                    | "}"
                    | "{"
            )
        {
            Ok(())
        } else {
            Err(self.error(Code::Syntax, "expected statement/declaration separator ';'"))
        }
    }
    fn separator(&mut self) -> Result<(), Diagnostic> {
        if self.eat(",") { Ok(()) } else { self.semi() }
    }
    fn binding(&mut self) -> Result<Binding, Diagnostic> {
        let kind = if self.eat("=") {
            BindingKind::Equal
        } else if self.eat("\\in") || self.eat("∈") {
            BindingKind::Member
        } else {
            return Err(self.error(Code::Syntax, "expected '=' or membership binder"));
        };
        Ok(Binding {
            kind,
            expression: self.expression(false, false)?,
        })
    }
    fn declaration(&mut self, parameter: bool) -> Result<Decl, Diagnostic> {
        self.charge(0)?;
        let name = self.name()?;
        let start = name.span.start;
        let initial = if matches!(self.peek(), "=" | "\\in" | "∈") {
            if parameter && self.peek() != "=" {
                return Err(self.error(
                    Code::Syntax,
                    "procedure parameter cannot use membership initialization",
                ));
            }
            Some(self.binding()?)
        } else {
            None
        };
        Ok(Decl {
            name,
            initial,
            span: Span {
                start,
                end: self.last(),
            },
        })
    }
    fn variables(&mut self, parameter: bool) -> Result<Vec<Decl>, Diagnostic> {
        if !self.eat("variable") && !self.eat("variables") {
            return Ok(Vec::new());
        }
        let mut variables = Vec::new();
        while !matches!(
            self.peek(),
            "begin" | "{" | "define" | "macro" | "procedure" | "process" | "fair" | ""
        ) {
            variables.push(self.declaration(parameter)?);
            self.separator()?;
        }
        Ok(variables)
    }
    fn begin(&mut self) -> Result<(), Diagnostic> {
        self.need(if self.syntax == Syntax::P {
            "begin"
        } else {
            "{"
        })
    }
    fn finish(&mut self, role: &str) -> Result<(), Diagnostic> {
        if self.syntax == Syntax::P {
            self.need("end")?;
            self.need(role)
        } else {
            self.need("}")
        }
    }
    fn algorithm(&mut self, start: usize, fair: bool) -> Result<Algorithm, Diagnostic> {
        self.charge(0)?;
        let name = self.name()?;
        if self.eat("{") {
            self.syntax = Syntax::C;
        }
        let variables = self.variables(false)?;
        let definitions = if self.eat("define") {
            if self.syntax == Syntax::C {
                self.need("{")?;
            }
            let expression = self.expression(true, true)?;
            self.finish("define")?;
            self.semi()?;
            Some(expression)
        } else {
            None
        };
        let mut macros = Vec::new();
        while self.peek() == "macro" {
            macros.push(self.macro_def()?);
        }
        let mut procedures = Vec::new();
        while self.peek() == "procedure" {
            procedures.push(self.procedure()?);
        }
        let body = if matches!(self.peek(), "process" | "fair") {
            let mut processes = Vec::new();
            while matches!(self.peek(), "process" | "fair") {
                processes.push(self.process()?);
            }
            Body::Processes(processes)
        } else {
            self.begin()?;
            let block = self.sequence(0, &["end", "}"], self.syntax == Syntax::C)?;
            if self.syntax == Syntax::C {
                self.need("}")?;
            }
            Body::Sequential(block)
        };
        self.finish("algorithm")?;
        self.eat(";");
        Ok(Algorithm {
            name,
            syntax: self.syntax,
            fairness: if fair {
                Fairness::Weak
            } else {
                Fairness::Unfair
            },
            variables,
            definitions,
            macros,
            procedures,
            body,
            span: Span {
                start,
                end: self.last(),
            },
        })
    }
    fn macro_def(&mut self) -> Result<Macro, Diagnostic> {
        self.charge(0)?;
        let start = self.start();
        self.need("macro")?;
        let name = self.name()?;
        self.need("(")?;
        let mut parameters = Vec::new();
        while self.peek() != ")" {
            parameters.push(self.name()?);
            if self.peek() != ")" {
                self.need(",")?;
            }
        }
        self.need(")")?;
        self.begin()?;
        self.in_macro = true;
        let body = self.sequence(0, &["end", "}"], self.syntax == Syntax::C)?;
        self.in_macro = false;
        self.finish("macro")?;
        self.eat(";");
        Ok(Macro {
            name,
            parameters,
            body,
            span: Span {
                start,
                end: self.last(),
            },
        })
    }
    fn procedure(&mut self) -> Result<Procedure, Diagnostic> {
        self.charge(0)?;
        let start = self.start();
        self.need("procedure")?;
        let name = self.name()?;
        self.need("(")?;
        let mut parameters = Vec::new();
        while self.peek() != ")" {
            parameters.push(self.declaration(true)?);
            if self.peek() != ")" {
                self.need(",")?;
            }
        }
        self.need(")")?;
        let variables = self.variables(true)?;
        self.begin()?;
        let body = self.sequence(0, &["end", "}"], self.syntax == Syntax::C)?;
        self.finish("procedure")?;
        self.eat(";");
        Ok(Procedure {
            name,
            parameters,
            variables,
            body,
            span: Span {
                start,
                end: self.last(),
            },
        })
    }
    fn process(&mut self) -> Result<Process, Diagnostic> {
        self.charge(0)?;
        let start = self.start();
        let fairness = if self.eat("fair") {
            if self.eat("+") {
                Fairness::Strong
            } else {
                Fairness::Weak
            }
        } else {
            Fairness::Unfair
        };
        self.need("process")?;
        if self.syntax == Syntax::C {
            self.need("(")?;
        }
        let name = self.name()?;
        let id = self.binding()?;
        if self.syntax == Syntax::C {
            self.need(")")?;
        }
        let variables = self.variables(false)?;
        self.begin()?;
        let body = self.sequence(0, &["end", "}"], self.syntax == Syntax::C)?;
        self.finish("process")?;
        self.eat(";");
        Ok(Process {
            name,
            fairness,
            id,
            variables,
            body,
            span: Span {
                start,
                end: self.last(),
            },
        })
    }

    fn expression(&mut self, define: bool, empty: bool) -> Result<Expr, Diagnostic> {
        let start = self.start();
        let first = self.pos;
        let mut depth = 0usize;
        let mut quantifier = false;
        let mut previous = " ".to_owned();
        while !self.peek().is_empty() {
            let word = self.peek();
            let field = matches!(previous.as_str(), "." | "[" | ",")
                && word
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_');
            let delimiter = matches!(
                word,
                ";" | "do"
                    | "then"
                    | ":="
                    | "≔"
                    | "begin"
                    | "variable"
                    | "variables"
                    | "||"
                    | "‖"
                    | "end"
                    | "else"
                    | "elsif"
                    | "if"
                    | "either"
                    | "or"
                    | "while"
                    | "with"
                    | "call"
                    | "return"
                    | "goto"
                    | "print"
                    | "assert"
                    | "skip"
                    | "procedure"
                    | "define"
                    | "process"
                    | "fair"
            ) && !field;
            if delimiter
                || (depth == 0 && matches!(word, ")" | "}"))
                || (depth == 0 && word == "," && !quantifier && !define)
            {
                if depth != 0 || quantifier {
                    return Err(self.error(
                        Code::Expression,
                        "expression has unmatched nesting or a quantifier without ':'",
                    ));
                }
                break;
            }
            if matches!(word, "(" | "[" | "{" | "<<" | "⟨") {
                depth += 1;
                if depth > self.opts.max_depth.min(256) {
                    return Err(
                        self.error(Code::Resource, "PlusCal expression nesting limit reached")
                    );
                }
            } else if matches!(word, ")" | "]" | "}" | ">>" | "⟩" | "]_" | ">>_" | "⟩_") {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    self.error(Code::Expression, "extra closing expression delimiter")
                })?;
            }
            if depth == 0 && matches!(word, "\\A" | "∀" | "\\E" | "∃") {
                quantifier = true;
            }
            if depth == 0 && word == ":" {
                quantifier = false;
            }
            previous = if word.starts_with('"') {
                " ".into()
            } else {
                word.to_owned()
            };
            self.pos += 1;
        }
        if depth != 0 || quantifier {
            return Err(self.error(
                Code::Expression,
                "unterminated expression nesting or quantifier",
            ));
        }
        if self.pos == first && !empty {
            return Err(self.error(Code::Expression, "missing expression"));
        }
        Ok(Expr {
            span: Span {
                start,
                end: if self.pos == first {
                    start
                } else {
                    self.last()
                },
            },
        })
    }
    fn label(&mut self) -> Result<Option<Label>, Diagnostic> {
        if self.at(1) != ":"
            || matches!(
                self.peek(),
                "while"
                    | "if"
                    | "else"
                    | "elsif"
                    | "either"
                    | "or"
                    | "end"
                    | "with"
                    | "when"
                    | "await"
                    | "skip"
                    | "call"
                    | "return"
                    | "goto"
                    | "assert"
                    | "print"
            )
        {
            return Ok(None);
        }
        let name = self.name()?;
        if self.in_macro || matches!(name.text.as_str(), "Done" | "Error") {
            return Err(Diagnostic {
                code: Code::Label,
                span: name.span,
                message: "reserved label or label inside a macro".into(),
            });
        }
        self.need(":")?;
        let modifier = if self.eat("+") {
            Some('+')
        } else if self.eat("-") {
            Some('-')
        } else {
            None
        };
        let span = Span {
            start: name.span.start,
            end: self.last(),
        };
        Ok(Some(Label {
            name,
            modifier,
            span,
        }))
    }
    fn sequence(
        &mut self,
        depth: usize,
        stops: &[&str],
        nonempty: bool,
    ) -> Result<Block, Diagnostic> {
        let start = self.start();
        let mut statements = Vec::new();
        while !self.peek().is_empty() && !stops.contains(&self.peek()) {
            statements.push(self.statement(depth + 1)?);
        }
        if nonempty && statements.is_empty() {
            return Err(self.error(Code::Syntax, "empty statement body"));
        }
        let end = if statements.is_empty() {
            start
        } else {
            self.last()
        };
        Ok(Block {
            span: Span { start, end },
            statements,
        })
    }
    fn c_body(&mut self, depth: usize) -> Result<Block, Diagnostic> {
        let start = self.start();
        let id = self.statement(depth + 1)?;
        Ok(Block {
            span: Span {
                start,
                end: self.last(),
            },
            statements: vec![id],
        })
    }
    fn arguments(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        self.need("(")?;
        let mut args = Vec::new();
        while self.peek() != ")" {
            args.push(self.expression(false, false)?);
            if self.peek() != ")" {
                self.need(",")?;
            }
        }
        self.need(")")?;
        Ok(args)
    }
    fn assignment(&mut self) -> Result<Assignment, Diagnostic> {
        self.charge(0)?;
        let variable = self.name()?;
        let start = variable.span.start;
        let selectors = self.expression(false, true)?;
        if !self.eat(":=") && !self.eat("≔") {
            return Err(self.error(Code::Syntax, "expected ':=' assignment token"));
        }
        let value = self.expression(false, false)?;
        Ok(Assignment {
            variable,
            selectors: if selectors.span.start == selectors.span.end {
                None
            } else {
                Some(selectors)
            },
            value,
            span: Span {
                start,
                end: self.last(),
            },
        })
    }
    // Keep the complete source-grammar dispatch together; each arm preserves a
    // distinct statement variant and recursively charges the common bounds.
    #[allow(clippy::too_many_lines)]
    fn statement(&mut self, depth: usize) -> Result<StmtId, Diagnostic> {
        self.charge(depth)?;
        let start = self.start();
        let label = self.label()?;
        let kind = match self.peek() {
            "{" if self.syntax == Syntax::C => {
                self.need("{")?;
                let mut body = self.sequence(depth, &["}"], true)?;
                self.need("}")?;
                self.semi()?;
                body.span = Span {
                    start,
                    end: self.last(),
                };
                if label.is_some()
                    && body
                        .statements
                        .first()
                        .and_then(|i| self.statements.get(i.0))
                        .is_some_and(|s| s.label.is_some())
                {
                    return Err(self.error(Code::Label, "duplicate labeling of statement block"));
                }
                StatementKind::Block(body)
            }
            "while" => {
                self.need("while")?;
                if self.syntax == Syntax::C {
                    self.need("(")?;
                }
                let test = self.expression(false, false)?;
                let body = if self.syntax == Syntax::P {
                    self.need("do")?;
                    let body = self.sequence(depth, &["end"], true)?;
                    self.finish("while")?;
                    self.semi()?;
                    body
                } else {
                    self.need(")")?;
                    self.c_body(depth)?
                };
                StatementKind::While { test, body }
            }
            "if" => {
                self.need("if")?;
                let mut branches = Vec::new();
                loop {
                    self.charge(depth)?;
                    if self.syntax == Syntax::C {
                        self.need("(")?;
                    }
                    let test = self.expression(false, false)?;
                    let body = if self.syntax == Syntax::P {
                        self.need("then")?;
                        self.sequence(depth, &["else", "elsif", "end"], false)?
                    } else {
                        self.need(")")?;
                        self.c_body(depth)?
                    };
                    branches.push(Branch { test, body });
                    if self.syntax == Syntax::P && self.eat("elsif") {
                        continue;
                    }
                    break;
                }
                let otherwise = if self.eat("else") {
                    Some(if self.syntax == Syntax::P {
                        self.sequence(depth, &["end"], false)?
                    } else {
                        self.c_body(depth)?
                    })
                } else {
                    None
                };
                if branches
                    .last()
                    .is_none_or(|branch| branch.body.statements.is_empty())
                    && otherwise
                        .as_ref()
                        .is_none_or(|body| body.statements.is_empty())
                {
                    return Err(self.error(Code::Syntax, "if statement has no branch statements"));
                }
                if self.syntax == Syntax::P {
                    self.finish("if")?;
                    self.semi()?;
                }
                StatementKind::If {
                    branches,
                    otherwise,
                }
            }
            "either" => {
                self.need("either")?;
                let mut branches = Vec::new();
                loop {
                    self.charge(depth)?;
                    branches.push(if self.syntax == Syntax::P {
                        self.sequence(depth, &["or", "end"], true)?
                    } else {
                        self.c_body(depth)?
                    });
                    if !self.eat("or") {
                        break;
                    }
                }
                if branches.len() < 2 {
                    return Err(self.error(Code::Syntax, "either requires an or clause"));
                }
                if self.syntax == Syntax::P {
                    self.finish("either")?;
                    self.semi()?;
                }
                StatementKind::Either(branches)
            }
            "with" => {
                self.need("with")?;
                if self.syntax == Syntax::C {
                    self.need("(")?;
                }
                let mut bindings = Vec::new();
                loop {
                    self.charge(depth)?;
                    let name = self.name()?;
                    let binding = self.binding()?;
                    bindings.push(WithBinding { name, binding });
                    if self.syntax == Syntax::C && self.peek() == ")" {
                        break;
                    }
                    self.separator()?;
                    if (self.syntax == Syntax::P && self.peek() == "do")
                        || (self.syntax == Syntax::C && self.peek() == ")")
                    {
                        break;
                    }
                }
                let body = if self.syntax == Syntax::P {
                    self.need("do")?;
                    let body = self.sequence(depth, &["end"], true)?;
                    self.finish("with")?;
                    self.semi()?;
                    body
                } else {
                    self.need(")")?;
                    self.c_body(depth)?
                };
                StatementKind::With { bindings, body }
            }
            "when" | "await" => {
                let when = self.eat("when");
                if !when {
                    self.need("await")?;
                }
                let condition = self.expression(false, false)?;
                self.semi()?;
                StatementKind::Await { when, condition }
            }
            "assert" => {
                self.need("assert")?;
                let e = self.expression(false, false)?;
                self.semi()?;
                StatementKind::Assert(e)
            }
            "print" => {
                self.need("print")?;
                let e = self.expression(false, false)?;
                self.semi()?;
                StatementKind::Print(e)
            }
            "skip" => {
                self.need("skip")?;
                self.semi()?;
                StatementKind::Skip
            }
            "return" => {
                self.need("return")?;
                self.semi()?;
                StatementKind::Return
            }
            "goto" => {
                self.need("goto")?;
                let name = self.name()?;
                self.semi()?;
                StatementKind::Goto(name)
            }
            "call" => {
                self.need("call")?;
                let name = self.name()?;
                let arguments = self.arguments()?;
                self.semi()?;
                StatementKind::Call { name, arguments }
            }
            _ if self.at(1) == "(" => {
                let name = self.name()?;
                let arguments = self.arguments()?;
                self.semi()?;
                StatementKind::MacroCall { name, arguments }
            }
            _ => {
                let mut assignments = vec![self.assignment()?];
                while self.eat("||") || self.eat("‖") {
                    assignments.push(self.assignment()?);
                }
                self.semi()?;
                StatementKind::Assign(assignments)
            }
        };
        let id = StmtId(self.statements.len());
        self.statements.push(Statement {
            span: Span {
                start,
                end: self.last(),
            },
            label,
            kind,
        });
        Ok(id)
    }
}
