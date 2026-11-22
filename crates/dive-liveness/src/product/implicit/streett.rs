//! Search positive Streett clauses without expanding their Cartesian product.
//! A candidate accepted by the residual tableau either satisfies every pair,
//! or violates one pair (FG p or GF q). Every real witness must then occur in
//! one of the two stricter searches. Empty subtrees prove absence of a witness.

use super::{
    Adjacency, CycleConditions, ImplicitProduct, Lasso, LassoSearch, ProductError, RepeatedRows,
    TruthAccess, csr,
};
use crate::formula::StreettPair;

struct Decision {
    pair: usize,
    recurring: bool,
}

fn violated(
    pairs: &[StreettPair],
    labels: &impl TruthAccess,
    word: &Lasso,
    scratch: &mut Vec<bool>,
) -> Result<Option<usize>, ProductError> {
    for (index, pair) in pairs.iter().enumerate() {
        csr::poll()?;
        let mut persistent = true;
        let mut recurring = false;
        for step in &word.cycle {
            csr::poll()?;
            let truth = |atom| labels.truth(atom, step.from, step.edge);
            persistent &= pair.persistent.evaluate_with_scratch(truth, scratch);
            if pair.recurring.evaluate_with_scratch(truth, scratch) {
                recurring = true;
                break;
            }
        }
        if !persistent && !recurring {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

impl<T: TruthAccess> ImplicitProduct<'_, T> {
    pub(crate) fn find_lasso_with_streett(
        &self,
        labels: &impl TruthAccess,
        alternatives: &[CycleConditions],
        pairs: &[StreettPair],
    ) -> Result<Option<Lasso>, ProductError> {
        if pairs.is_empty() {
            return self.find_lasso_in_alternatives(labels, alternatives);
        }
        if let Some(rows) = RepeatedRows::build(self, crate::budget::optional_buffer_limit())? {
            return self.refine_streett(&rows, labels, alternatives, pairs);
        }
        self.refine_streett(self, labels, alternatives, pairs)
    }

    fn refine_streett(
        &self,
        graph: &impl Adjacency,
        labels: &impl TruthAccess,
        alternatives: &[CycleConditions],
        pairs: &[StreettPair],
    ) -> Result<Option<Lasso>, ProductError> {
        let all = CycleConditions {
            always: pairs.iter().map(|p| p.persistent.clone()).collect(),
            recurring: pairs.iter().map(|p| p.recurring.clone()).collect(),
        };
        let scratch = || {
            super::super::solution_scratch(
                &self.acceptance,
                labels,
                &all,
                self.policy,
                || self.admit_cells(&all),
                &self.solution_setup_work,
            )
        };
        // Check all predicate references and admission even on an empty graph.
        drop(scratch()?);
        let mut search = LassoSearch::new(graph);
        let mut decisions = csr::reserve::<Decision>(pairs.len())?;
        for base in alternatives {
            let mut conditions = base.clone();
            csr::grow(&mut conditions.always, pairs.len())?;
            csr::grow(&mut conditions.recurring, pairs.len())?;
            loop {
                csr::poll()?;
                if let Some(word) =
                    self.search_alternative(labels, &conditions, &mut search, false)?
                {
                    let Some(pair) = violated(pairs, labels, &word, &mut scratch()?)? else {
                        return Ok(Some(word));
                    };
                    if decisions.iter().any(|d| d.pair == pair) {
                        return Err(ProductError::Shape(
                            "chosen Streett condition failed on its own witness".into(),
                        ));
                    }
                    decisions.push(Decision {
                        pair,
                        recurring: true,
                    });
                    conditions.recurring.push(pairs[pair].recurring.clone());
                    continue;
                }
                // Backtrack the single live condition vector. The explicit
                // stack holds at most one bit of branch state per pair.
                let mut sibling = false;
                while let Some(decision) = decisions.pop() {
                    csr::poll()?;
                    if decision.recurring {
                        conditions.recurring.pop();
                        conditions
                            .always
                            .push(pairs[decision.pair].persistent.clone());
                        decisions.push(Decision {
                            recurring: false,
                            ..decision
                        });
                        sibling = true;
                        break;
                    }
                    conditions.always.pop();
                }
                if !sibling {
                    break;
                }
            }
        }
        Ok(None)
    }
}
