//! Sequential generator matching `PcalTLAGen`'s changed-variable and branch rules.
use super::lower::{Kind, Model, Node, tokens};
use super::{
    Translation, TranslationError, TranslationMap, TranslationOptions, TranslationStage, error,
};
use crate::{
    Code, Parsed,
    ast::{Assignment, BindingKind, Expr, Span},
};
use std::collections::BTreeSet;

type Changed = BTreeSet<String>;
struct Emitter<'a> {
    parsed: &'a Parsed,
    model: &'a Model,
    limit: usize,
}
fn failure(span: Span, code: Code, message: impl Into<String>) -> TranslationError {
    error(TranslationStage::Generation, code, span, message)
}
fn conjunction(clauses: &[String], force: bool) -> String {
    if clauses.is_empty() {
        return "TRUE".into();
    }
    if clauses.len() == 1 && !force {
        return clauses[0].clone();
    }
    clauses
        .iter()
        .map(|clause| format!("/\\ {}", clause.replace('\n', "\n   ")))
        .collect::<Vec<_>>()
        .join("\n")
}
impl Emitter<'_> {
    fn bounded(&self, text: String, span: Span) -> Result<String, TranslationError> {
        if text.len() > self.limit {
            return Err(failure(
                span,
                Code::Resource,
                "PlusCal generated-byte limit reached",
            ));
        }
        Ok(text)
    }
    fn variables(&self) -> Vec<&str> {
        let mut vars = Vec::new();
        if self.model.use_pc {
            vars.push("pc");
        }
        vars.extend(
            self.model
                .variables
                .iter()
                .map(|var| var.name.text.as_str()),
        );
        vars
    }
    fn unchanged(names: &[&str]) -> Option<String> {
        match names {
            [] => None,
            [name] => Some(format!("{name}' = {name}")),
            _ => Some(format!("UNCHANGED << {} >>", names.join(", "))),
        }
    }
    fn expression(&self, expr: Expr, changed: &Changed) -> Result<String, TranslationError> {
        if self.parsed.source[expr.span.start..expr.span.end].contains(['\n', '\r']) {
            return Err(failure(
                expr.span,
                Code::Unsupported,
                "unsupported PlusCal translation: multiline TLA expression layout/substitution",
            ));
        }
        let source = tokens(self.parsed, expr);
        // The reference performs lexical substitution, not a scope-aware TLA
        // rewrite. Binding constructs need a separate proved shadowing path.
        if source.iter().any(|token| {
            matches!(
                token.text.as_str(),
                "LET" | "\\A" | "\\E" | "∀" | "∃" | "CHOOSE" | "LAMBDA"
            )
        }) {
            return Err(failure(
                expr.span,
                Code::Unsupported,
                "unsupported PlusCal translation: TLA expression binder substitution",
            ));
        }
        if source
            .iter()
            .any(|token| matches!(token.text.as_str(), "\\in" | "∈"))
            && source
                .iter()
                .any(|token| matches!(token.text.as_str(), "|->" | "↦"))
            || source.iter().enumerate().any(|(index, token)| {
                token.text == ":"
                    && (index < 2 || !matches!(source[index - 2].text.as_str(), "[" | ","))
            })
        {
            return Err(failure(
                expr.span,
                Code::Unsupported,
                "unsupported PlusCal translation: function/set expression binder substitution",
            ));
        }
        let mut words = Vec::new();
        for (index, token) in source.iter().enumerate() {
            let field = index > 0 && source[index - 1].text == ".";
            let record_key = index > 0
                && matches!(source[index - 1].text.as_str(), "[" | ",")
                && source
                    .get(index + 1)
                    .is_some_and(|next| matches!(next.text.as_str(), ":" | "|->" | "↦"));
            if !field && !record_key && changed.contains(&token.text) {
                words.push(format!("{}'", token.text));
            } else {
                words.push(token.text.clone());
            }
        }
        self.bounded(words.join(" "), expr.span)
    }
    fn value_expression(&self, expr: Expr, changed: &Changed) -> Result<String, TranslationError> {
        let text = self.expression(expr, changed)?;
        let first_line_end = self.parsed.source[expr.span.start..expr.span.end]
            .find(['\n', '\r'])
            .map_or(expr.span.end, |offset| expr.span.start + offset);
        let mut depth: i32 = 0;
        let mut needs = false;
        // NeedsParentheses scans the first normalized line repeatedly (the
        // pinned implementation uses vec.elementAt(0)); preserve that quirk.
        for token in tokens(self.parsed, expr) {
            if token.span.start >= first_line_end {
                break;
            }
            let word = token.text.as_str();
            match word {
                "(" | "[" | "{" | "<<" => depth += 1,
                ")" | "]" | "}" | ">>" => {
                    if depth == 0 {
                        needs = true;
                    }
                    depth -= 1;
                }
                _ if depth == 0
                    && (matches!(
                        word,
                        "=" | "#"
                            | "<"
                            | ">"
                            | "<="
                            | ">="
                            | "=>"
                            | "<=>"
                            | "|-"
                            | "-|"
                            | "|->"
                            | "/\\"
                    ) || (word.starts_with('\\')
                        && !word.starts_with("\\o")
                        && !word.starts_with("\\X"))) =>
                {
                    needs = true;
                }
                _ => {}
            }
        }
        if needs {
            self.bounded(format!("({text})"), expr.span)
        } else {
            Ok(text)
        }
    }
    fn assigns(
        &self,
        assignments: &[Assignment],
        changed: &mut Changed,
    ) -> Result<Vec<String>, TranslationError> {
        let mut ordered: Vec<_> = assignments.iter().collect();
        ordered.sort_by(|left, right| {
            left.variable
                .text
                .encode_utf16()
                .cmp(right.variable.text.encode_utf16())
        });
        let before = changed.clone();
        let mut clauses = Vec::new();
        let mut index = 0;
        while index < ordered.len() {
            let assignment = ordered[index];
            let name = assignment.variable.text.as_str();
            if !self.model.variables.iter().any(|var| var.name.text == name) {
                return Err(failure(
                    assignment.variable.span,
                    Code::Syntax,
                    format!("assignment to undeclared variable {name}"),
                ));
            }
            if before.contains(name) {
                return Err(failure(
                    assignment.span,
                    Code::Label,
                    format!("missing label before repeated assignment to {name}"),
                ));
            }
            let end = ordered[index..].partition_point(|item| item.variable.text == name) + index;
            let group = &ordered[index..end];
            let rhs = if group.len() == 1 && assignment.selectors.is_none() {
                self.value_expression(assignment.value, &before)?
            } else {
                let mut fields = Vec::new();
                for item in group {
                    let selectors = item.selectors.ok_or_else(|| {
                        failure(
                            item.span,
                            Code::Syntax,
                            "multiple whole-variable assignments in parallel assignment",
                        )
                    })?;
                    fields.push(format!(
                        "!{} = {}",
                        self.expression(selectors, &before)?,
                        self.expression(item.value, &before)?
                    ));
                }
                self.bounded(
                    format!("[{name} EXCEPT {}]", fields.join(", ")),
                    assignment.span,
                )?
            };
            clauses.push(format!("{name}' = {rhs}"));
            changed.insert(name.into());
            index = end;
        }
        Ok(clauses)
    }
    fn branches(
        &self,
        branches: &[&[Node]],
        before: &Changed,
    ) -> Result<(Vec<Vec<String>>, Changed), TranslationError> {
        let mut output = Vec::new();
        let mut after = Vec::new();
        let mut union = before.clone();
        for branch in branches {
            let mut changed = before.clone();
            let clauses = self.sequence(branch, &mut changed)?;
            union.extend(changed.iter().cloned());
            output.push(clauses);
            after.push(changed);
        }
        let vars = self.variables();
        for (clauses, changed) in output.iter_mut().zip(after) {
            let missing: Vec<_> = vars
                .iter()
                .copied()
                .filter(|name| union.contains(*name) && !changed.contains(*name))
                .collect();
            if let Some(identity) = Self::unchanged(&missing) {
                clauses.push(identity);
            }
        }
        Ok((output, union))
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one bounded statement-kind dispatcher retains changed-variable sequencing"
    )]
    fn sequence(
        &self,
        nodes: &[Node],
        changed: &mut Changed,
    ) -> Result<Vec<String>, TranslationError> {
        let mut clauses = Vec::new();
        let mut bytes = 0usize;
        for node in nodes {
            let previous = clauses.len();
            match &node.kind {
                Kind::Assign(assignments) => {
                    let assignments = self.assigns(assignments, changed)?;
                    if assignments.len() > 1 {
                        clauses.push(conjunction(&assignments, true));
                    } else {
                        clauses.extend(assignments);
                    }
                }
                Kind::Pc(target) => {
                    if changed.contains("pc") {
                        return Err(failure(
                            node.span,
                            Code::Label,
                            "multiple program-counter updates in one action",
                        ));
                    }
                    clauses.push(format!("pc' = {}", serde_string(target)));
                    changed.insert("pc".into());
                }
                Kind::Await(expr) => clauses.push(self.expression(*expr, changed)?),
                Kind::Assert(expr) => {
                    // Labels belong to the statement source span, but the
                    // reference diagnostic points at the assertion keyword.
                    let keyword = self
                        .parsed
                        .tokens
                        .iter()
                        .find(|token| {
                            token.span.start >= node.span.start
                                && token.span.end <= node.span.end
                                && token.text == "assert"
                        })
                        .ok_or_else(|| {
                            failure(
                                node.span,
                                Code::Internal,
                                "assertion keyword missing from source span",
                            )
                        })?;
                    let position = self.parsed.position(keyword.span.start).ok_or_else(|| {
                        failure(
                            node.span,
                            Code::Internal,
                            "invalid assertion source position",
                        )
                    })?;
                    clauses.push(format!(
                        "Assert({}, \"Failure of assertion at line {}, column {}.\")",
                        self.expression(*expr, changed)?,
                        position.line,
                        position.column
                    ));
                }
                Kind::Print(expr) => {
                    clauses.push(format!("PrintT({})", self.expression(*expr, changed)?));
                }
                Kind::Skip => clauses.push("TRUE".into()),
                Kind::If { test, yes, no } => {
                    let test = self.expression(*test, changed)?;
                    let (branches, union) =
                        self.branches(&[yes.as_slice(), no.as_slice()], changed)?;
                    let yes = conjunction(&branches[0], true);
                    let no = conjunction(&branches[1], true);
                    clauses.push(format!(
                        "IF {test}\n   THEN {}\n   ELSE {}",
                        yes.replace('\n', "\n        "),
                        no.replace('\n', "\n        ")
                    ));
                    *changed = union;
                }
                Kind::Either(branches) => {
                    let (branches, union) = self.branches(
                        &branches.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                        changed,
                    )?;
                    clauses.push(
                        branches
                            .iter()
                            .map(|branch| {
                                format!("\\/ {}", conjunction(branch, true).replace('\n', "\n   "))
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                    *changed = union;
                }
                Kind::With { bindings, body } => {
                    let before = changed.clone();
                    let body = self.sequence(body, changed)?;
                    let mut text = conjunction(&body, false);
                    for binding in bindings.iter().rev() {
                        let expr = self.expression(binding.binding.expression, &before)?;
                        let head = match binding.binding.kind {
                            BindingKind::Equal => format!("LET {} == {expr} IN", binding.name.text),
                            BindingKind::Member => {
                                format!("\\E {} \\in {expr}:", binding.name.text)
                            }
                        };
                        text = self.bounded(
                            format!("{head}\n   {}", text.replace('\n', "\n   ")),
                            node.span,
                        )?;
                    }
                    clauses.push(text);
                }
            }
            bytes =
                bytes.saturating_add(clauses[previous..].iter().map(String::len).sum::<usize>());
            if bytes > self.limit {
                return Err(failure(
                    node.span,
                    Code::Resource,
                    "PlusCal generated-byte limit reached",
                ));
            }
        }
        Ok(clauses)
    }
}
fn serde_string(text: &str) -> String {
    // Label spellings are identifier tokens or the source spelling "Done".
    if text.starts_with('"') {
        text.into()
    } else {
        format!("\"{text}\"")
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "ordered artifact sections preserve the pinned generator order and bounded source mapping"
)]
pub(super) fn generate(
    parsed: &Parsed,
    model: &Model,
    options: &TranslationOptions,
) -> Result<Translation, TranslationError> {
    let emitter = Emitter {
        parsed,
        model,
        limit: options.max_output_bytes.min(4_194_304),
    };
    let span = parsed.algorithm.span;
    let names = emitter.variables();
    let mut text = String::new();
    let mut mappings = Vec::new();
    let mut append = |fragment: String, original: Span| -> Result<(), TranslationError> {
        if text.len().saturating_add(fragment.len()) > emitter.limit {
            return Err(failure(
                original,
                Code::Resource,
                "PlusCal generated-byte limit reached",
            ));
        }
        let start = text.len();
        text.push_str(&fragment);
        mappings.push(TranslationMap {
            generated: Span {
                start,
                end: text.len(),
            },
            original,
        });
        Ok(())
    };
    let keyword = if names.len() == 1 {
        "VARIABLE"
    } else {
        "VARIABLES"
    };
    append(
        format!(
            "{keyword} {}\n\nvars == << {} >>\n\n",
            names.join(", "),
            names.join(", ")
        ),
        span,
    )?;
    let mut init = Vec::new();
    for variable in &model.variables {
        if let Some(binding) = &variable.initial {
            let op = if binding.kind == BindingKind::Equal {
                "="
            } else {
                "\\in"
            };
            init.push(format!(
                "{} {op} {}",
                variable.name.text,
                emitter.value_expression(binding.expression, &Changed::new())?
            ));
        }
    }
    if model.use_pc {
        init.push(format!("pc = {}", serde_string(&model.actions[0].name)));
    }
    append(
        format!(
            "Init == {}\n\n",
            conjunction(&init, true).replace('\n', "\n        ")
        ),
        span,
    )?;
    for action in &model.actions {
        let mut changed = Changed::new();
        let mut body = Vec::new();
        if model.use_pc {
            body.push(format!("pc = {}", serde_string(&action.name)));
        }
        body.extend(emitter.sequence(&action.body, &mut changed)?);
        let missing: Vec<_> = names
            .iter()
            .copied()
            .filter(|name| !changed.contains(*name))
            .collect();
        if let Some(identity) = Emitter::unchanged(&missing) {
            body.push(identity);
        }
        let name = if model.use_pc {
            action.name.as_str()
        } else {
            "Next"
        };
        append(
            format!(
                "{name} == {}\n\n",
                conjunction(&body, body.len() > 1)
                    .replace('\n', &format!("\n{}", " ".repeat(name.len() + 4)))
            ),
            action.span,
        )?;
    }
    if model.stutter {
        append(
            "Terminating == pc = \"Done\" /\\ UNCHANGED vars\n\n".into(),
            span,
        )?;
    }
    if model.use_pc {
        let mut actions: Vec<_> = model
            .actions
            .iter()
            .map(|action| action.name.as_str())
            .collect();
        if model.stutter {
            actions.push("Terminating");
        }
        append(format!("Next == {}\n\n", actions.join(" \\/ ")), span)?;
    }
    let spec = if model.fair {
        "/\\ Init /\\ [][Next]_vars\n        /\\ WF_vars(Next)"
    } else {
        "Init /\\ [][Next]_vars"
    };
    append(format!("Spec == {spec}\n\n"), span)?;
    if model.stutter {
        append("Termination == <>(pc = \"Done\")\n".into(), span)?;
    }
    // Counted source expressions can be malformed even when the PlusCal
    // source scanner accepts them. Never return a syntactically invalid body.
    let module = format!("---- MODULE NativePlusCal ----\nEXTENDS TLC\n{text}\n====\n");
    let result = dive_syntax::parse_module(&module);
    if !result.is_ok() {
        return Err(failure(
            span,
            Code::Expression,
            "generated TLA syntax rejected; source expression validation failed",
        ));
    }
    Ok(Translation {
        tla: text,
        mappings,
        actions: model
            .actions
            .iter()
            .map(|action| action.name.clone())
            .collect(),
        program_counter: model.use_pc,
        warnings: model.warnings.clone(),
    })
}
