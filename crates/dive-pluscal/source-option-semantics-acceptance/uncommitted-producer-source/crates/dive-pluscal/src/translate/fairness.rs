//! Bounded source-backed `PcalFixIDs` reachability and `GenSpec` grouping.
use super::{TranslationError, TranslationStage, error, lower::Model, macros::Expanded};
use crate::{
    Code,
    ast::{BindingKind, Fairness, Label, Process, Span, StatementKind},
};
use std::collections::BTreeMap;

fn resource(span: Span) -> TranslationError {
    error(
        TranslationStage::Generation,
        Code::Resource,
        span,
        "PlusCal fairness construction limit reached",
    )
}
fn append(
    text: &mut String,
    piece: &str,
    limit: usize,
    span: Span,
) -> Result<(), TranslationError> {
    if piece.len() > limit.saturating_sub(text.len()) {
        return Err(resource(span));
    }
    text.push_str(piece);
    Ok(())
}
fn charge(
    work: &mut usize,
    amount: usize,
    limit: usize,
    span: Span,
) -> Result<(), TranslationError> {
    if amount > limit.saturating_sub(*work) {
        return Err(resource(span));
    }
    *work += amount;
    Ok(())
}
fn calls(
    parsed: &Expanded,
    span: Span,
    names: &BTreeMap<&str, usize>,
    work: &mut usize,
    limit: usize,
) -> Result<Vec<usize>, TranslationError> {
    let mut calls = Vec::new();
    for statement in &parsed.statements {
        charge(work, 1, limit, span)?;
        if statement.span.start >= span.start
            && statement.span.end <= span.end
            && let StatementKind::Call { name, .. } = &statement.kind
            && let Some(index) = names.get(name.text.as_str())
        {
            calls.push((statement.span.start, *index));
        }
    }
    calls.sort_unstable();
    let mut result = Vec::new();
    let mut seen = vec![false; names.len()];
    for (_, index) in calls {
        if !seen[index] {
            seen[index] = true;
            result.push(index);
        }
    }
    Ok(result)
}
fn reachable(
    parsed: &Expanded,
    model: &Model,
    process: &Process,
    work: &mut usize,
    limit: usize,
) -> Result<Vec<usize>, TranslationError> {
    let names: BTreeMap<_, _> = model
        .procedures
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.text.as_str(), i))
        .collect();
    let mut result = calls(parsed, process.body.span, &names, work, limit)?;
    if result.is_empty() {
        return Ok(result);
    }
    let graph: Vec<_> = model
        .procedures
        .iter()
        .map(|p| calls(parsed, p.body.span, &names, work, limit))
        .collect::<Result<_, _>>()?;
    let mut present = vec![false; graph.len()];
    for &index in &result {
        present[index] = true;
    }
    // Source direct-call order survives. Each closure is appended in declaration
    // order; appended entries cannot add a vertex outside those closures.
    let direct = result.clone();
    for index in direct {
        let mut seen = vec![false; graph.len()];
        let mut queue = Vec::new();
        for &next in &graph[index] {
            if !seen[next] {
                seen[next] = true;
                queue.push(next);
            }
        }
        let mut cursor = 0;
        while cursor < queue.len() {
            let current = queue[cursor];
            cursor += 1;
            for &next in &graph[current] {
                charge(work, 1, limit, process.span)?;
                if !seen[next] {
                    seen[next] = true;
                    queue.push(next);
                }
            }
        }
        for (next, yes) in seen.into_iter().enumerate() {
            if yes && !present[next] {
                present[next] = true;
                result.push(next);
            }
        }
    }
    Ok(result)
}
struct Pair {
    wf: String,
    sf: Option<String>,
}
impl Pair {
    fn width(&self) -> usize {
        self.wf.encode_utf16().count()
            + self.sf.as_ref().map_or(0, |s| 4 + s.encode_utf16().count())
    }
    fn single(&self, text: &mut String, limit: usize, span: Span) -> Result<(), TranslationError> {
        append(text, &self.wf, limit, span)?;
        if let Some(sf) = &self.sf {
            append(text, " /\\ ", limit, span)?;
            append(text, sf, limit, span)?;
        }
        Ok(())
    }
    fn multi(
        &self,
        text: &mut String,
        col: usize,
        limit: usize,
        span: Span,
    ) -> Result<(), TranslationError> {
        append(text, "/\\ ", limit, span)?;
        append(text, &self.wf, limit, span)?;
        if let Some(sf) = &self.sf {
            append(text, "\n", limit, span)?;
            append(text, &" ".repeat(col), limit, span)?;
            append(text, "/\\ ", limit, span)?;
            append(text, sf, limit, span)?;
        }
        Ok(())
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "source ownership, fairness and budgets stay explicit at one formula boundary"
)]
fn pair(
    parsed: &Expanded,
    span: Span,
    fairness: Fairness,
    name: &str,
    parameter: &str,
    owner: &str,
    work: &mut usize,
    limit: usize,
) -> Result<Pair, TranslationError> {
    let mut labels: Vec<&Label> = Vec::new();
    for statement in &parsed.statements {
        charge(work, 1, limit, span)?;
        if statement.span.start >= span.start
            && statement.span.end <= span.end
            && let Some(label) = &statement.label
        {
            labels.push(label);
        }
    }
    labels.sort_by_key(|label| label.span.start);
    let minus: Vec<_> = labels.iter().filter(|l| l.modifier == Some('-')).collect();
    let mut wf = String::new();
    append(
        &mut wf,
        if fairness == Fairness::Strong {
            "SF_vars("
        } else {
            "WF_vars("
        },
        limit,
        span,
    )?;
    if !minus.is_empty() {
        append(&mut wf, "(pc[", limit, span)?;
        append(&mut wf, owner, limit, span)?;
        append(
            &mut wf,
            if minus.len() == 1 {
                "] # \""
            } else {
                "] \\notin {\""
            },
            limit,
            span,
        )?;
        for (index, label) in minus.iter().enumerate() {
            if index > 0 {
                append(&mut wf, "\", \"", limit, span)?;
            }
            append(&mut wf, &label.name.text, limit, span)?;
        }
        append(
            &mut wf,
            if minus.len() == 1 {
                "\") /\\ "
            } else {
                "\"}) /\\ "
            },
            limit,
            span,
        )?;
    }
    append(&mut wf, name, limit, span)?;
    append(&mut wf, parameter, limit, span)?;
    append(&mut wf, ")", limit, span)?;
    let mut sf = String::new();
    if fairness == Fairness::Weak {
        for label in labels.into_iter().filter(|l| l.modifier == Some('+')) {
            if !sf.is_empty() {
                append(&mut sf, " /\\ ", limit, span)?;
            }
            append(&mut sf, "SF_vars(", limit, span)?;
            append(&mut sf, &label.name.text, limit, span)?;
            append(&mut sf, parameter, limit, span)?;
            append(&mut sf, ")", limit, span)?;
        }
    }
    Ok(Pair {
        wf,
        sf: (!sf.is_empty()).then_some(sf),
    })
}
#[allow(
    clippy::too_many_lines,
    reason = "one bounded port retains source formula order and significant junction grouping"
)]
pub(super) fn formula(
    parsed: &Expanded,
    model: &Model,
    process: &Process,
    limit: usize,
    work: &mut usize,
) -> Result<Option<String>, TranslationError> {
    if process.fairness == Fairness::Unfair {
        return Ok(None);
    }
    let owner = if process.id.kind == BindingKind::Member {
        "self".to_owned()
    } else {
        super::lower::tokens(parsed, process.id.expression)
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let parameter = if process.id.kind == BindingKind::Member {
        "(self)"
    } else {
        ""
    };
    let body = pair(
        parsed,
        process.body.span,
        process.fairness,
        &process.name.text,
        parameter,
        &owner,
        work,
        limit,
    )?;
    let mut retained = body.wf.len() + body.sf.as_ref().map_or(0, String::len);
    if retained > limit {
        return Err(resource(process.span));
    }
    let mut procedures = Vec::new();
    let argument = format!("({owner})");
    for index in reachable(parsed, model, process, work, limit)? {
        let procedure = &model.procedures[index];
        let p = pair(
            parsed,
            procedure.body.span,
            process.fairness,
            &procedure.name.text,
            &argument,
            &owner,
            work,
            limit,
        )?;
        charge(
            &mut retained,
            p.wf.len() + p.sf.as_ref().map_or(0, String::len) + 4,
            limit,
            process.span,
        )?;
        procedures.push(p);
    }
    let mut prefix = String::new();
    if process.id.kind == BindingKind::Member {
        let source = parsed.text(process.id.expression.span).unwrap_or_default();
        append(&mut prefix, "\\A self \\in ", limit, process.span)?;
        append(&mut prefix, source, limit, process.span)?;
        append(&mut prefix, " : ", limit, process.span)?;
    }
    // Pinned singleLineWidth omits a single prefix line and the body's /\
    // separator. Those quirks affect conjunction grouping and are preserved.
    let width = body.wf.encode_utf16().count()
        + body.sf.as_ref().map_or(0, |s| s.encode_utf16().count())
        + procedures.iter().map(Pair::width).sum::<usize>();
    let mut text = prefix.clone();
    if 11 + width <= 78 || body.sf.is_none() && procedures.is_empty() {
        body.single(&mut text, limit, process.span)?;
        for p in &procedures {
            append(&mut text, " /\\ ", limit, process.span)?;
            p.single(&mut text, limit, process.span)?;
        }
    } else {
        let col = 11 + prefix.encode_utf16().count();
        if col + body.width() + 3 <= 78 {
            append(&mut text, "/\\ ", limit, process.span)?;
            body.single(&mut text, limit, process.span)?;
        } else {
            body.multi(&mut text, col, limit, process.span)?;
        }
        for (index, p) in procedures.iter().enumerate() {
            if index == 0 {
                append(&mut text, "\n", limit, process.span)?;
            }
            append(&mut text, &" ".repeat(col), limit, process.span)?;
            if col + p.width() + 3 <= 78 {
                append(&mut text, "/\\ ", limit, process.span)?;
                p.single(&mut text, limit, process.span)?;
                append(&mut text, "\n", limit, process.span)?;
            } else {
                p.multi(&mut text, col, limit, process.span)?;
            }
        }
    }
    // GenSpec Java String.split drops only trailing empty lines.
    while text.ends_with('\n') {
        text.pop();
    }
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Body;
    #[test]
    #[allow(
        clippy::expect_used,
        reason = "source admission and bounded resource results are test assertions"
    )]
    fn aggregate_fairness_work_is_charged_before_next_owner() {
        let source = "--algorithm A { variable x=0; procedure Add(a=0) { F:x:=x+a; R:return; }; fair process (One=1) { L:call Add(1); E:skip; } fair process (Two=2) { M:call Add(2); N:skip; } fair process (Three=3) { O:call Add(3); P:skip; } }";
        let parsed = crate::parse(source, &crate::Options::default()).expect("bounded source");
        let parsed =
            super::super::macros::expand(parsed, &super::super::TranslationOptions::default())
                .expect("no macros");
        assert!(matches!(parsed.algorithm.body, Body::Processes(_)));
        let Body::Processes(processes) = &parsed.algorithm.body else {
            return;
        };
        let model = Model {
            actions: Vec::new(),
            variables: Vec::new(),
            use_pc: true,
            stutter: true,
            fair: false,
            warnings: Vec::new(),
            processes: processes.clone(),
            procedures: parsed.algorithm.procedures.clone(),
            initial_pc: None,
        };
        let mut work = 0;
        assert!(
            formula(&parsed, &model, &processes[0], 50, &mut work)
                .expect("first owner fits")
                .is_some()
        );
        let failure = formula(&parsed, &model, &processes[1], 50, &mut work)
            .expect_err("next owner shares work quota");
        assert_eq!(failure.diagnostic.code, Code::Resource);
        assert!(work <= 50);
    }
}
