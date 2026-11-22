//! Bounded repair of concrete, named counterexamples against a newly bound Next.

use super::{Check, Ending, Report, SpecModel, State, Trace, TraceDoc, Validation};
use dive_eval::next::Flow;
use dive_eval::{Host, HostError, St};
use dive_ir::StdOp;
use dive_model::Model;
use dive_runtime::budget::{Budget, EvaluationControl, Phase};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

const WORD_LIMIT: usize = 4096;
const EDGE_LIMIT: usize = 32_768;
const GAP_LIMIT: usize = 128;
const DEPTH_LIMIT: usize = 4;

/// Result of checking a saved word, with a repaired document only after a fresh replay.
#[derive(Debug)]
pub struct WordRepair {
    /// Validation of the supplied word, or of the complete returned replacement.
    pub validation: Report,
    /// A validated replacement. An unchanged donor can be used directly.
    pub replacement: Option<TraceDoc>,
    /// Why the search declined or the final independent replay rejected the candidate.
    pub reason: Option<String>,
    /// Independent failure on a proposed replacement, when one reached replay.
    pub rejected: Option<Report>,
    /// Actual complete or partial successor occurrences generated in short-path searches.
    pub generated: usize,
    /// Original edges replaced by paths in the candidate.
    pub gaps: usize,
    /// Milliseconds spent checking the donor and, when constructed, its replacement.
    pub validation_ms: f64,
    /// Milliseconds preparing and searching for a replacement, exclusive of final replay.
    pub search_ms: f64,
}

#[derive(Debug)]
struct PureHost;
impl Host for PureHost {
    fn check_native_access(&self, op: StdOp) -> Result<(), HostError> {
        if op.impure() {
            return Err(HostError {
                code: 0,
                message: format!("{} during witness repair", op.qualified_name()),
            });
        }
        Ok(())
    }
}

impl SpecModel {
    /// Revalidates a stored native lasso; on a failed Next step, tries bounded
    /// paths between its concrete anchors using the current source actions.
    /// Any replacement is independently replayed on the entire specification.
    /// A caller can run its ordinary check whenever validation does not succeed.
    ///
    /// # Errors
    /// Returns a diagnostic on validation execution failure. An unsuccessful optional
    /// search retains the original validation with its reason in the returned value.
    pub fn repair_word(&self, doc: &TraceDoc) -> Result<WordRepair, String> {
        if doc.source != dive_trace::Source::Dive
            || !matches!(doc.ending, Some(Ending::Lasso { .. } | Ending::Stuttering))
            || doc.properties.is_empty()
        {
            return Err("repair requires a named native periodic counterexample".into());
        }
        if doc.states.len() > WORD_LIMIT {
            return Err("repair input exceeds 4096 states".into());
        }
        let start = Instant::now();
        let validation = self.try_validate(doc).map_err(|e| e.to_string())?;
        let mut result = WordRepair {
            validation,
            replacement: None,
            reason: None,
            rejected: None,
            generated: 0,
            gaps: 0,
            validation_ms: start.elapsed().as_secs_f64() * 1e3,
            search_ms: 0.,
        };
        if !matches!(&result.validation.validation, Validation::Invalid(f) if f.check == Check::Next)
        {
            return Ok(result);
        }
        let start = Instant::now();
        let built =
            super::on_big_stack(|| self.bridge(doc, &mut result)).map_err(|e| e.to_string())?;
        result.search_ms = start.elapsed().as_secs_f64() * 1e3;
        match built {
            Ok(candidate) => {
                let start = Instant::now();
                let replay = self.try_validate(&candidate).map_err(|e| e.to_string())?;
                result.validation_ms += start.elapsed().as_secs_f64() * 1e3;
                if replay.validation == Validation::Valid {
                    result.validation = replay;
                    result.replacement = Some(candidate);
                } else {
                    result.reason = Some("replacement failed independent validation".into());
                    result.rejected = Some(replay);
                }
            }
            Err(reason) => result.reason = Some(reason),
        }
        Ok(result)
    }

    fn bridge(&self, doc: &TraceDoc, result: &mut WordRepair) -> Result<TraceDoc, String> {
        if doc.labels.iter().any(Option::is_some)
            || doc.witnesses.iter().any(|row| !row.is_empty())
            || doc.draws.iter().any(|row| !row.is_empty())
            || !doc.startup_draws.is_empty()
            || doc.run.is_some()
        {
            return Err("repair requires a named, unlabeled native periodic word within 4096 states and without recorded draws or witnesses".into());
        }
        let mut watchdog = Budget::new(Some(Duration::from_secs(30)), Some(512 << 20))
            .start()
            .map_err(|e| e.to_string())?
            .ok_or("missing repair watchdog")?;
        let budget = watchdog.budget();
        let control = EvaluationControl::new(budget, Phase::Search).map_err(|e| e.to_string())?;
        // A private evaluator owns generation caches. The separate replay reconstructs
        // its own runtime from the original source and never consumes these caches.
        let m = self
            .recipe
            .bind_next_fallible(Arc::new(PureHost), Some(control.clone()))
            .map_err(|e| e.to_string())?;
        let _scope = control.enter(&m.evaluator).map_err(|e| e.to_string())?;
        let next = m.next.as_ref().ok_or("missing next-state relation")?;
        let mut word = Trace::decode(&m, doc).map_err(|e| e.detail)?;
        let old = std::mem::take(&mut word.states);
        let last = old.len().checked_sub(1).ok_or("empty word")?;
        let back = match word.ending {
            Ending::Lasso { back_to } if back_to <= last => back_to,
            Ending::Stuttering => last,
            _ => return Err("invalid loop target".into()),
        };
        let mut new_back = 0;
        for (i, source) in old.iter().enumerate() {
            if i == back {
                new_back = word.states.len();
            }
            word.states.push(source.clone());
            let target = &old[if i == last { back } else { i + 1 }];
            if source == target {
                continue;
            }
            let pair = St::action(source.clone(), target.clone());
            if !m.eval_boolean(next, &pair).map_err(|e| e.to_string())? {
                if result.gaps >= GAP_LIMIT {
                    return Err("repair gap limit".into());
                }
                let intermediate = short_path(&m, source, target, result, budget)?;
                if word.states.len() + intermediate.len() + last - i > WORD_LIMIT {
                    return Err("repaired word exceeds 4096 states".into());
                }
                word.states.extend(intermediate);
                result.gaps += 1;
            }
        }
        if matches!(word.ending, Ending::Lasso { .. }) {
            word.ending = Ending::Lasso { back_to: new_back };
        }
        word.length = Some(word.states.len());
        word.labels = vec![None; word.states.len()];
        word.witnesses.clear();
        word.draws.clear();
        let out = word.to_doc(&m)?;
        if watchdog.finish().map_err(|e| e.to_string())?.is_some() {
            return Err("repair time or memory limit".into());
        }
        Ok(out)
    }
}

fn short_path(
    m: &Model,
    source: &State,
    target: &State,
    result: &mut WordRepair,
    budget: &Budget,
) -> Result<Vec<State>, String> {
    // Parent offsets point into a bounded BFS vector. The goal itself stays
    // the donor's next anchor and is never inserted twice.
    let mut nodes = vec![(source.clone(), 0, 0)];
    #[allow(
        clippy::mutable_key_type,
        reason = "execution values are immutable; interning and cached hashes preserve their semantic equality"
    )]
    let mut visited = HashSet::from([source.clone()]);
    let mut cursor = 0;
    while cursor < nodes.len() {
        let (state, _, depth) = nodes[cursor].clone();
        let mut found = false;
        let mut limit = None;
        m.evaluator
            .successors(&m.actions, &state, &mut |succ| {
                if result.generated >= EDGE_LIMIT || !matches!(budget.poll(), Ok(None)) {
                    limit = Some("repair successor, time or memory limit");
                    return Flow::Stop;
                }
                result.generated += 1;
                if !succ.complete() {
                    return Flow::Continue;
                }
                if succ.state == target.as_ref() {
                    found = true;
                    return Flow::Stop;
                }
                if depth + 1 < DEPTH_LIMIT && !visited.contains(succ.state) {
                    if nodes.len() >= WORD_LIMIT {
                        limit = Some("repair frontier limit");
                        return Flow::Stop;
                    }
                    let row: State = Arc::from(succ.state);
                    visited.insert(row.clone());
                    nodes.push((row, cursor, depth + 1));
                }
                Flow::Continue
            })
            .map_err(|e| e.error.to_string())?;
        if let Some(why) = limit {
            return Err(why.into());
        }
        if found {
            if cursor == 0 {
                return Err(
                    "generated direct step disagrees with the predicate on the anchors".into(),
                );
            }
            let mut path = Vec::new();
            while cursor != 0 {
                path.push(nodes[cursor].0.clone());
                cursor = nodes[cursor].1;
            }
            path.reverse();
            return Ok(path);
        }
        cursor += 1;
    }
    Err("no path within four current Next steps".into())
}
