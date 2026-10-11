//! Owner-aware generator matching `PcalTLAGen`'s changed-variable and branch rules.
use super::lower::{Kind, Model, Node, tokens};
use super::macros::Expanded as Parsed;
use super::{
    Translation, TranslationError, TranslationMap, TranslationOptions, TranslationStage, error,
};
use crate::{
    Code,
    ast::{Assignment, BindingKind, Expr, Span},
};
use std::collections::BTreeSet;

type Changed = BTreeSet<String>;
struct Emitter<'a> {
    parsed: &'a Parsed,
    model: &'a Model,
    limit: usize,
    process: Option<&'a crate::ast::Process>,
    procedure: bool,
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
    fn procedure_slot(&self, name: &str) -> bool {
        !self.model.processes.is_empty()
            && !self.model.procedures.is_empty()
            && (matches!(name, "pc" | "stack")
                || self.model.procedures.iter().any(|p| {
                    p.parameters
                        .iter()
                        .chain(&p.variables)
                        .any(|d| d.name.text == name)
                }))
    }
    fn local(&self, name: &str) -> bool {
        self.procedure_slot(name)
            || self.process.is_some_and(|p| {
                p.id.kind == BindingKind::Member && p.variables.iter().any(|v| v.name.text == name)
            })
    }
    fn read(&self, name: &str, changed: &Changed) -> String {
        format!(
            "{name}{}{}",
            if changed.contains(name) { "'" } else { "" },
            if self.local(name) {
                format!("[{}]", self.process_id())
            } else {
                String::new()
            }
        )
    }
    fn update(&self, name: &str, value: &str) -> String {
        if self.local(name) {
            format!(
                "{name}' = [{name} EXCEPT ![{}] = {value}]",
                self.process_id()
            )
        } else {
            format!("{name}' = {value}")
        }
    }
    fn process_id(&self) -> String {
        if self.procedure {
            return "self".into();
        }
        self.process.map_or_else(String::new, |p| {
            if p.id.kind == BindingKind::Member {
                "self".into()
            } else {
                tokens(self.parsed, p.id.expression)
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        })
    }
    fn domain(&self) -> String {
        self.process.map_or_else(String::new, |p| {
            tokens(self.parsed, p.id.expression)
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
    }
    fn parameter(&self) -> &'static str {
        if self.procedure
            || self
                .process
                .is_some_and(|p| p.id.kind == BindingKind::Member)
        {
            "(self)"
        } else {
            ""
        }
    }
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
        if !self.model.procedures.is_empty() {
            vars.insert(
                usize::from(self.model.use_pc) + self.parsed.algorithm.variables.len(),
                "stack",
            );
        }
        vars
    }
    fn unchanged(names: &[&str]) -> Option<String> {
        match names {
            [] => None,
            [name] => Some(format!("{name}' = {name}")),
            _ => Some(format!("UNCHANGED << {} >>", names.join(", "))),
        }
    }
    fn expression_layout(
        &self,
        expr: Expr,
        source: &[crate::ast::Token],
    ) -> Result<(), TranslationError> {
        let text = self.parsed.text(expr.span).unwrap_or_default();
        if !text.contains(['\n', '\r']) {
            return Ok(());
        }
        // TLA junction-list columns are semantic. Flattening their source
        // lines can change nesting, so retain this unproved layout boundary.
        if source.iter().any(|token| {
            matches!(
                token.text.as_str(),
                "/\\" | "\\/" | "∧" | "∨" | "\\land" | "\\lor"
            )
        }) || text
            .as_bytes()
            .iter()
            .enumerate()
            .any(|(index, byte)| *byte == b'\r' && text.as_bytes().get(index + 1) != Some(&b'\n'))
            || self.parsed.declared_macros()
            || !self.model.processes.is_empty()
            || !self.model.procedures.is_empty()
        {
            return Err(failure(
                expr.span,
                Code::Unsupported,
                "unsupported PlusCal translation: multiline junction/macro/process/procedure expression layout",
            ));
        }
        // In the admitted non-junction slice, TLA token order and existing
        // changed-variable substitutions define the same expression tree.
        // Newline trivia is omitted; reference pretty-print bytes are not claimed.
        Ok(())
    }
    fn expression(&self, expr: Expr, changed: &Changed) -> Result<String, TranslationError> {
        let source = tokens(self.parsed, expr);
        self.expression_layout(expr, source)?;
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
            if !field
                && !record_key
                && self.local(&token.text)
                && !matches!(token.text.as_str(), "stack" | "pc")
            {
                words.push(format!(
                    "{}{}[{}]",
                    token.text,
                    if changed.contains(&token.text) {
                        "'"
                    } else {
                        ""
                    },
                    self.process_id()
                ));
            } else if !field && !record_key && changed.contains(&token.text) {
                words.push(format!("{}'", token.text));
            } else {
                words.push(token.text.clone());
            }
        }
        self.bounded(words.join(" "), expr.span)
    }
    fn value_expression(&self, expr: Expr, changed: &Changed) -> Result<String, TranslationError> {
        let text = self.expression(expr, changed)?;
        let first_line_end = if expr.span.start > self.parsed.source.len() {
            usize::MAX
        } else {
            self.parsed
                .text(expr.span)
                .unwrap_or_default()
                .find(['\n', '\r'])
                .map_or(expr.span.end, |offset| expr.span.start + offset)
        };
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
                            | "->"
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
            let rhs = if self.local(name) {
                let fields = group
                    .iter()
                    .map(|item| {
                        let selectors = item.selectors.map_or_else(
                            || Ok(String::new()),
                            |expr| self.expression(expr, &before),
                        )?;
                        let value = if item.selectors.is_none() {
                            self.value_expression(item.value, &before)?
                        } else {
                            self.expression(item.value, &before)?
                        };
                        Ok(format!("![{}]{selectors} = {value}", self.process_id()))
                    })
                    .collect::<Result<Vec<_>, TranslationError>>()?;
                self.bounded(
                    format!("[{name} EXCEPT {}]", fields.join(", ")),
                    assignment.span,
                )?
            } else if group.len() == 1 && assignment.selectors.is_none() {
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
    #[allow(
        clippy::too_many_lines,
        reason = "one source-backed simultaneous CallReturn assignment phase followed by callee initialization"
    )]
    fn call_return(
        &self,
        from: usize,
        to: usize,
        arguments: &[Expr],
        changed: &mut Changed,
        span: Span,
    ) -> Result<Vec<String>, TranslationError> {
        let owner = self
            .model
            .procedures
            .get(from)
            .ok_or_else(|| failure(span, Code::Internal, "tail return owner missing"))?;
        let target = self
            .model
            .procedures
            .get(to)
            .ok_or_else(|| failure(span, Code::Internal, "tail call target missing"))?;
        if changed.contains("stack") || changed.contains("pc") {
            return Err(failure(
                span,
                Code::Label,
                "multiple procedure transfers in one action",
            ));
        }
        let before = changed.clone();
        let mut assignments = Vec::new();
        // ExplodeCallReturn preserves the existing frame for a self tail
        // call. A cross-procedure tail call restores caller locals and
        // replaces the frame head before binding the callee parameters.
        if from != to {
            for local in &owner.variables {
                let name = &local.name.text;
                if changed.contains(name) {
                    return Err(failure(
                        span,
                        Code::Label,
                        "missing label before tail restoring local",
                    ));
                }
                assignments.push((
                    name.clone(),
                    self.update(
                        name,
                        &format!("Head({}).{name}", self.read("stack", &Changed::new())),
                    ),
                ));
                changed.insert(name.clone());
            }
            let mut fields = vec![
                format!("procedure |-> {}", serde_string(&target.name.text)),
                format!("pc |-> Head({}).pc", self.read("stack", &Changed::new())),
            ];
            for decl in target.variables.iter().chain(&target.parameters) {
                let name = &decl.name.text;
                fields.push(format!("{name} |-> {}", self.read(name, &before)));
            }
            assignments.push((
                "stack".into(),
                self.update(
                    "stack",
                    &format!(
                        "<< [ {} ] >> \\o Tail({})",
                        fields.join(", "),
                        self.read("stack", &Changed::new())
                    ),
                ),
            ));
            changed.insert("stack".into());
        }
        for (parameter, argument) in target.parameters.iter().zip(arguments) {
            let name = &parameter.name.text;
            if changed.contains(name) {
                return Err(failure(
                    span,
                    Code::Label,
                    "missing label before repeated tail parameter assignment",
                ));
            }
            assignments.push((
                name.clone(),
                self.update(name, &self.value_expression(*argument, &before)?),
            ));
            changed.insert(name.clone());
        }
        assignments.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        let assignments: Vec<_> = assignments.into_iter().map(|(_, value)| value).collect();
        // Pinned GenAssign emits an empty conjunction for a zero-argument
        // self tail call. Retain the raw invalid reference artifact here;
        // public module admission rejects it, rather than silently repairing
        // a source-backed generation behavior into an admitted artifact.
        let mut clauses = vec![if assignments.is_empty() {
            String::new()
        } else {
            conjunction(&assignments, assignments.len() > 1)
        }];
        for local in &target.variables {
            let name = &local.name.text;
            if changed.contains(name) {
                return Err(failure(
                    span,
                    Code::Label,
                    "missing label before tail local initialization",
                ));
            }
            let value = if let Some(binding) = &local.initial {
                self.value_expression(binding.expression, changed)?
            } else {
                "defaultInitValue".into()
            };
            clauses.push(self.update(name, &value));
            changed.insert(name.clone());
        }
        let entry = self
            .model
            .actions
            .iter()
            .find(|a| a.span.start >= target.body.span.start && a.span.end <= target.body.span.end)
            .ok_or_else(|| failure(span, Code::Internal, "tail procedure entry missing"))?;
        clauses.push(self.update("pc", &serde_string(&entry.name)));
        changed.insert("pc".into());
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
                Kind::Call {
                    procedure,
                    arguments,
                    next,
                } => {
                    let procedure = self.model.procedures.get(*procedure).ok_or_else(|| {
                        failure(
                            node.span,
                            Code::Internal,
                            "normalized procedure target missing",
                        )
                    })?;
                    if changed.contains("stack") || changed.contains("pc") {
                        return Err(failure(
                            node.span,
                            Code::Label,
                            "multiple procedure transfers in one action",
                        ));
                    }
                    let before = changed.clone();
                    let mut fields = vec![
                        format!("procedure |-> {}", serde_string(&procedure.name.text)),
                        format!("pc |-> {}", serde_string(next)),
                    ];
                    for decl in procedure.variables.iter().chain(&procedure.parameters) {
                        let name = &decl.name.text;
                        fields.push(format!("{name} |-> {}", self.read(name, &before)));
                    }
                    let mut assignments = Vec::new();
                    for (parameter, argument) in procedure.parameters.iter().zip(arguments) {
                        let name = &parameter.name.text;
                        if changed.contains(name) {
                            return Err(failure(
                                node.span,
                                Code::Label,
                                "missing label before repeated procedure parameter assignment",
                            ));
                        }
                        assignments.push((
                            name.clone(),
                            self.update(name, &self.value_expression(*argument, &before)?),
                        ));
                        changed.insert(name.clone());
                    }
                    assignments.push((
                        "stack".into(),
                        self.update(
                            "stack",
                            &format!(
                                "<< [ {} ] >> \\o {}",
                                fields.join(", "),
                                self.read("stack", &Changed::new())
                            ),
                        ),
                    ));
                    assignments.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
                    let assignments: Vec<_> =
                        assignments.into_iter().map(|(_, value)| value).collect();
                    clauses.push(conjunction(&assignments, assignments.len() > 1));
                    changed.insert("stack".into());
                    for local in &procedure.variables {
                        let name = &local.name.text;
                        if changed.contains(name) {
                            return Err(failure(
                                node.span,
                                Code::Label,
                                "missing label before procedure local initialization",
                            ));
                        }
                        let value = if let Some(binding) = &local.initial {
                            self.value_expression(binding.expression, changed)?
                        } else {
                            "defaultInitValue".into()
                        };
                        clauses.push(self.update(name, &value));
                        changed.insert(name.clone());
                    }
                    let entry = self
                        .model
                        .actions
                        .iter()
                        .find(|a| {
                            a.span.start >= procedure.body.span.start
                                && a.span.end <= procedure.body.span.end
                        })
                        .ok_or_else(|| {
                            failure(
                                procedure.span,
                                Code::Internal,
                                "procedure entry action missing",
                            )
                        })?;
                    clauses.push(self.update("pc", &serde_string(&entry.name)));
                    changed.insert("pc".into());
                }
                Kind::CallReturn {
                    from,
                    to,
                    arguments,
                } => {
                    clauses.extend(self.call_return(*from, *to, arguments, changed, node.span)?);
                }
                Kind::Return(procedure) => {
                    let procedure = self.model.procedures.get(*procedure).ok_or_else(|| {
                        failure(node.span, Code::Internal, "normalized return owner missing")
                    })?;
                    if changed.contains("stack") || changed.contains("pc") {
                        return Err(failure(
                            node.span,
                            Code::Label,
                            "multiple procedure transfers in one action",
                        ));
                    }
                    clauses.push(self.update(
                        "pc",
                        &format!("Head({}).pc", self.read("stack", &Changed::new())),
                    ));
                    changed.insert("pc".into());
                    for decl in procedure.variables.iter().chain(&procedure.parameters) {
                        let name = &decl.name.text;
                        if changed.contains(name) {
                            return Err(failure(
                                node.span,
                                Code::Label,
                                "missing label before restoring procedure variable",
                            ));
                        }
                        clauses.push(self.update(
                            name,
                            &format!("Head({}).{name}", self.read("stack", &Changed::new())),
                        ));
                        changed.insert(name.clone());
                    }
                    clauses.push(self.update(
                        "stack",
                        &format!("Tail({})", self.read("stack", &Changed::new())),
                    ));
                    changed.insert("stack".into());
                }
                Kind::Pc(target) => {
                    if changed.contains("pc") {
                        return Err(failure(
                            node.span,
                            Code::Label,
                            "multiple program-counter updates in one action",
                        ));
                    }
                    clauses.push(if self.process.is_some() || self.procedure {
                        format!(
                            "pc' = [pc EXCEPT ![{}] = {}]",
                            self.process_id(),
                            serde_string(target)
                        )
                    } else {
                        format!("pc' = {}", serde_string(target))
                    });
                    changed.insert("pc".into());
                }
                Kind::Await(expr) => clauses.push(self.expression(*expr, changed)?),
                Kind::Assert(expr) => {
                    if let Some(location) = self.parsed.assertion_location(node.span, *expr) {
                        clauses.push(format!(
                            "Assert({}, \"Failure of assertion at {}.\")",
                            self.expression(*expr, changed)?,
                            location
                        ));
                        continue;
                    }
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
                    if yes.is_empty() {
                        return Err(failure(
                            node.span,
                            Code::Unsupported,
                            "unsupported PlusCal translation: empty THEN branch layout",
                        ));
                    }
                    let test = self.expression(*test, changed)?;
                    let (mut branches, union) =
                        self.branches(&[yes.as_slice(), no.as_slice()], changed)?;
                    // PcalTLAGen.GenIf retains TRUE for an empty Else before
                    // adding the branch identity updates. Preserve its AST.
                    if no.is_empty() {
                        branches[1].insert(0, "TRUE".into());
                    }
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
        process: model.processes.first(),
        procedure: false,
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
    if model.variables.iter().any(|var| var.initial.is_none()) {
        append("CONSTANT defaultInitValue\n\n".into(), span)?;
    }
    append(
        format!(
            "{keyword} {}\n\nvars == << {} >>\n\n",
            names.join(", "),
            names.join(", ")
        ),
        span,
    )?;
    if !model.processes.is_empty() {
        let domains: Vec<_> = model
            .processes
            .iter()
            .map(|process| {
                let owner = Emitter {
                    process: Some(process),
                    ..emitter
                };
                let domain = owner.domain();
                if process.id.kind == BindingKind::Equal {
                    format!("{{{domain}}}")
                } else {
                    format!("({domain})")
                }
            })
            .collect();
        append(format!("ProcSet == {}\n\n", domains.join(" \\cup ")), span)?;
    }
    let default_init = model.variables.iter().any(|var| var.initial.is_none());
    let mut init = Vec::new();
    for variable in &model.variables {
        let emitter = Emitter {
            process: model.processes.iter().find(|p| {
                p.variables
                    .iter()
                    .any(|v| v.name.text == variable.name.text)
            }),
            procedure: emitter.procedure_slot(&variable.name.text),
            ..emitter
        };
        let domain = if emitter.procedure {
            "ProcSet".into()
        } else {
            emitter.domain()
        };
        if let Some(binding) = &variable.initial {
            let op = if binding.kind == BindingKind::Equal {
                "="
            } else {
                "\\in"
            };
            let value = emitter.value_expression(binding.expression, &Changed::new())?;
            let value = if emitter.local(&variable.name.text) {
                if binding.kind == BindingKind::Equal {
                    format!("[self \\in {domain} |-> {value}]")
                } else {
                    format!("[{domain} -> {value}]")
                }
            } else {
                value
            };
            init.push(format!("{} {op} {value}", variable.name.text));
        } else {
            let value = if emitter.local(&variable.name.text) {
                format!("[self \\in {domain} |-> defaultInitValue]")
            } else {
                "defaultInitValue".into()
            };
            init.push(format!("{} = {value}", variable.name.text));
        }
    }
    if !model.procedures.is_empty() {
        init.push(if model.processes.is_empty() {
            "stack = << >>".into()
        } else {
            "stack = [self \\in ProcSet |-> << >>]".into()
        });
    }
    if model.use_pc {
        let value = serde_string(
            model
                .initial_pc
                .as_deref()
                .unwrap_or(&model.actions[0].name),
        );
        init.push(if model.processes.len() > 1 {
            let arms: Vec<_> = model
                .processes
                .iter()
                .map(|process| {
                    let owner = Emitter {
                        process: Some(process),
                        ..emitter
                    };
                    let first = model
                        .actions
                        .iter()
                        .find(|a| {
                            a.span.start >= process.span.start && a.span.end <= process.span.end
                        })
                        .map_or("", |a| a.name.as_str());
                    let op = if process.id.kind == BindingKind::Equal {
                        "="
                    } else {
                        "\\in"
                    };
                    format!("self {op} {} -> {}", owner.domain(), serde_string(first))
                })
                .collect();
            format!("pc = [self \\in ProcSet |-> CASE {}]", arms.join(" [] "))
        } else if model.processes.is_empty() {
            format!("pc = {value}")
        } else {
            format!("pc = [self \\in ProcSet |-> {value}]")
        });
    }
    append(
        format!(
            "Init == {}\n\n",
            conjunction(&init, true).replace('\n', "\n        ")
        ),
        span,
    )?;
    for (index, action) in model.actions.iter().enumerate() {
        let emitter = Emitter {
            procedure: !model.processes.is_empty()
                && model.procedures.iter().any(|p| {
                    action.span.start >= p.body.span.start && action.span.end <= p.body.span.end
                }),
            process: model
                .processes
                .iter()
                .find(|p| action.span.start >= p.span.start && action.span.end <= p.span.end),
            ..emitter
        };
        let mut changed = Changed::new();
        let mut body = Vec::new();
        if model.use_pc {
            body.push(if model.processes.is_empty() {
                format!("pc = {}", serde_string(&action.name))
            } else {
                format!(
                    "pc[{}] = {}",
                    emitter.process_id(),
                    serde_string(&action.name)
                )
            });
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
            format!("{}{}", action.name, emitter.parameter())
        } else if let Some(process) = emitter.process {
            format!("{}{}", process.name.text, emitter.parameter())
        } else {
            "Next".into()
        };
        append(
            format!(
                "{name} == {}\n\n",
                conjunction(&body, body.len() > 1)
                    .replace('\n', &format!("\n{}", " ".repeat(name.len() + 4)))
            ),
            action.span,
        )?;
        if let Some(procedure) = model
            .procedures
            .iter()
            .find(|p| action.span.start >= p.body.span.start && action.span.end <= p.body.span.end)
            && model
                .actions
                .get(index + 1)
                .is_none_or(|next| next.span.start >= procedure.body.span.end)
        {
            let actions: Vec<_> = model
                .actions
                .iter()
                .filter(|a| {
                    a.span.start >= procedure.body.span.start
                        && a.span.end <= procedure.body.span.end
                })
                .map(|a| format!("{}{}", a.name, emitter.parameter()))
                .collect();
            append(
                format!(
                    "{}{} == {}\n\n",
                    procedure.name.text,
                    emitter.parameter(),
                    actions.join(" \\/ ")
                ),
                procedure.span,
            )?;
        }
        if model.use_pc
            && let Some(process) = emitter.process
            && model
                .actions
                .get(index + 1)
                .is_none_or(|next| next.span.start >= process.span.end)
        {
            append(
                format!(
                    "{}{} == {}\n\n",
                    process.name.text,
                    emitter.parameter(),
                    model
                        .actions
                        .iter()
                        .filter(|a| a.span.start >= process.span.start
                            && a.span.end <= process.span.end)
                        .map(|a| format!("{}{}", a.name, emitter.parameter()))
                        .collect::<Vec<_>>()
                        .join(" \\/ ")
                ),
                process.span,
            )?;
        }
    }
    if model.stutter {
        append(
            if model.processes.is_empty() {
                "Terminating == pc = \"Done\" /\\ UNCHANGED vars\n\n".into()
            } else {
                "Terminating == /\\ \\A self \\in ProcSet: pc[self] = \"Done\"\n               /\\ UNCHANGED vars\n\n".into()
            },
            span,
        )?;
    }
    if !model.processes.is_empty() {
        // GenNext emits constant-ID wrappers before quantified process sets,
        // even when the source interleaves the two kinds of declarations.
        let mut actions: Vec<_> = model
            .processes
            .iter()
            .filter(|p| p.id.kind == BindingKind::Equal)
            .map(|p| p.name.text.clone())
            .chain(
                model
                    .processes
                    .iter()
                    .filter(|p| p.id.kind == BindingKind::Member)
                    .map(|process| {
                        let owner = Emitter {
                            process: Some(process),
                            ..emitter
                        };
                        format!(
                            "(\\E self \\in {}: {}(self))",
                            owner.domain(),
                            process.name.text
                        )
                    }),
            )
            .collect();
        if !model.procedures.is_empty() {
            let at = model
                .processes
                .iter()
                .filter(|p| p.id.kind == BindingKind::Equal)
                .count();
            actions.insert(
                at,
                format!(
                    "(\\E self \\in ProcSet: {})",
                    model
                        .procedures
                        .iter()
                        .map(|p| format!("{}(self)", p.name.text))
                        .collect::<Vec<_>>()
                        .join(" \\/ ")
                ),
            );
        }
        append(
            format!(
                "Next == {}{}\n\n",
                actions.join(" \\/ "),
                if model.stutter {
                    " \\/ Terminating"
                } else {
                    ""
                }
            ),
            span,
        )?;
    } else if model.use_pc {
        let mut actions: Vec<_> = model
            .actions
            .iter()
            .filter(|action| {
                !model.procedures.iter().any(|p| {
                    action.span.start >= p.body.span.start && action.span.end <= p.body.span.end
                })
            })
            .map(|action| action.name.as_str())
            .collect();
        let mut procedures: Vec<_> = model
            .procedures
            .iter()
            .map(|p| p.name.text.as_str())
            .collect();
        procedures.append(&mut actions);
        let mut actions = procedures;
        if model.stutter {
            actions.push("Terminating");
        }
        append(format!("Next == {}\n\n", actions.join(" \\/ ")), span)?;
    }
    let mut spec = if model.fair
        || model
            .processes
            .iter()
            .any(|p| p.fairness != crate::ast::Fairness::Unfair)
    {
        "/\\ Init /\\ [][Next]_vars".to_owned()
    } else {
        "Init /\\ [][Next]_vars".into()
    };
    if model.fair {
        spec.push_str("\n        /\\ WF_vars(Next)");
    }
    let mut fairness_work = 0;
    for process in &model.processes {
        if let Some(formula) =
            super::fairness::formula(parsed, model, process, emitter.limit, &mut fairness_work)?
        {
            if 12 + formula.len() > emitter.limit.saturating_sub(spec.len()) {
                return Err(failure(
                    process.span,
                    Code::Resource,
                    "PlusCal generated-byte limit reached across fairness clauses",
                ));
            }
            spec.push_str("\n        /\\ ");
            spec.push_str(&formula);
        }
    }
    append(format!("Spec == {spec}\n\n"), span)?;
    if model.stutter {
        append(
            if model.processes.is_empty() {
                "Termination == <>(pc = \"Done\")\n".into()
            } else {
                "Termination == <>(\\A self \\in ProcSet: pc[self] = \"Done\")\n".into()
            },
            span,
        )?;
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
        inserted_labels: Vec::new(),
        configuration: if default_init {
            "CONSTANT defaultInitValue = defaultInitValue\n".into()
        } else {
            String::new()
        },
        processes: model
            .processes
            .iter()
            .map(|process| super::ProcessTranslation {
                name: process.name.text.clone(),
                binding_kind: process.id.kind,
                id_source: process.id.expression.span,
                local_variables: process
                    .variables
                    .iter()
                    .map(|var| var.name.text.clone())
                    .collect(),
                source: process.span,
            })
            .collect(),
    })
}
