//! Reached query dispatch for ordinary trace predicates. Normal interpreter
//! evaluation handles data, short circuit, argument forcing and host behavior;
//! only an actual ENABLED occurrence enters the independent finite search.

use std::sync::{Arc, Mutex, PoisonError, Weak};

use dive_eval::{EnabledResolver, EvalError, Scope, St};
use dive_ir::ExprId;
use dive_model::{Model, Pred};

use crate::exists::{ExistsError, Limits};

#[derive(Debug, Default)]
struct Work {
    expressions: usize,
    assignments: usize,
    invocation: Option<St>,
}

#[derive(Debug)]
struct Queries {
    model: Weak<Model>,
    work: Mutex<Work>,
    limits: Limits,
}

impl EnabledResolver for Queries {
    fn resolve(&self, scope: &Scope, action: ExprId, st: &St) -> dive_eval::Result<bool> {
        let model = self
            .model
            .upgrade()
            .ok_or_else(|| EvalError::unsupported("independent query invocation has ended"))?;
        let mut work = self.work.lock().unwrap_or_else(PoisonError::into_inner);
        // The independent search strips this callback, handles nested queries
        // itself and shares its counters. It cannot re-enter this mutex.
        let supplied = st.without_enabled_resolver();
        let invocation = if let Some(invocation) = &work.invocation {
            model
                .evaluator
                .share_pure_predicate_budget(&supplied, invocation)?
        } else {
            let invocation = model
                .evaluator
                .begin_pure_predicate(scope, &supplied, 4096)?;
            work.invocation = Some(invocation.clone());
            invocation
        };
        let Work {
            expressions,
            assignments,
            ..
        } = &mut *work;
        let result = crate::exists::reached_query(
            &model,
            &Pred::new(action, scope.clone()),
            &invocation,
            self.limits,
            expressions,
            assignments,
        );
        result.map_err(|error| match error {
            ExistsError::Invalid(_) => EvalError::general(error.to_string()),
            ExistsError::Unsupported(_) => EvalError::unsupported(error.to_string()),
            ExistsError::Limit(_) => EvalError::resource_limit(error.to_string()),
        })
    }
}

/// The model owner lives only for this validation run. Captured invocation
/// states retain a weak model reference, so caches cannot form a Core cycle.
pub(crate) struct OrdinaryQueries {
    model: Arc<Model>,
    queries: Arc<Queries>,
}

impl OrdinaryQueries {
    pub(crate) fn new(model: &Model) -> Self {
        let model = Arc::new(model.clone());
        let queries = Arc::new(Queries {
            model: Arc::downgrade(&model),
            work: Mutex::default(),
            limits: Limits::default(),
        });
        Self { model, queries }
    }

    pub(crate) fn eval(&self, pred: &Pred, st: &St) -> dive_eval::Result<dive_value::Value> {
        *self
            .queries
            .work
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Work::default();
        let st = self
            .model
            .evaluator
            .with_enabled_resolver(st, self.queries.clone())?;
        self.model.eval(pred, &st)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "owned runtime and budget controls")]
mod tests {
    use super::*;
    use dive_value::Value;

    fn model() -> Arc<Model> {
        let analysis = dive_sema::analyze_source(
            "Reached.tla",
            "---- MODULE Reached ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit==x=0\nNext==x'=x\nBoth==ENABLED (x'=7) /\\ ENABLED (x'=8)\nHidden(p)==<<p>>[1]\nLazy==Hidden(ENABLED (x'=7))\nSkip==FALSE /\\ ENABLED (x'>0)\nEarlier==1\\div 0=0 /\\ ENABLED (x'>0)\nLater==ENABLED (x'>0) /\\ 1\\div 0=0\n====\n",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        Arc::new(
            dive_model::bind(
                Arc::new(dive_ir::lower(&analysis).expect("IR")),
                &dive_config::parse("INIT Init\nNEXT Next\n")
                    .into_config()
                    .expect("config"),
                &dive_model::BindOptions::tlc(),
            )
            .expect("model"),
        )
    }

    fn pred(model: &Model, name: &str) -> Pred {
        let def = model.ir.root_def(name).expect(name);
        Pred::new(
            model.ir.def(def).body,
            model.evaluator.scope(def).expect("scope"),
        )
    }

    #[test]
    fn reached_sibling_queries_share_assignments_and_expression_work() {
        let model = model();
        let st = St::state(Arc::from([Some(0.into())]));
        for (limits, succeeds) in [
            (
                Limits {
                    assignments: 2,
                    ..Limits::default()
                },
                true,
            ),
            (
                Limits {
                    assignments: 1,
                    ..Limits::default()
                },
                false,
            ),
            (
                Limits {
                    expressions: 0,
                    ..Limits::default()
                },
                false,
            ),
        ] {
            let queries = Arc::new(Queries {
                model: Arc::downgrade(&model),
                work: Mutex::default(),
                limits,
            });
            let supplied = model
                .evaluator
                .with_enabled_resolver(&st, queries.clone())
                .expect("invocation");
            let result = model.eval(&pred(&model, "Both"), &supplied);
            if succeeds {
                assert_eq!(result.expect("two independent queries"), Value::TRUE);
                assert_eq!(queries.work.lock().expect("work").assignments, 2);
            } else {
                assert_eq!(
                    result.expect_err("no budget replenishment").class(),
                    dive_eval::ErrorClass::ResourceLimit
                );
            }
        }
    }

    #[test]
    fn normal_interpreter_reaches_hidden_data_queries_and_preserves_error_order() {
        let model = model();
        let queries = OrdinaryQueries::new(&model);
        let st = St::state(Arc::from([Some(0.into())]));
        assert_eq!(
            queries
                .eval(&pred(&model, "Lazy"), &st)
                .expect("actual data-hidden query"),
            Value::TRUE
        );
        assert_eq!(
            queries
                .eval(&pred(&model, "Skip"), &st)
                .expect("unreached unsupported relation"),
            Value::FALSE
        );
        assert_eq!(
            queries
                .eval(&pred(&model, "Earlier"), &st)
                .expect_err("earlier division error")
                .class(),
            dive_eval::ErrorClass::Eval
        );
        assert_eq!(
            queries
                .eval(&pred(&model, "Later"), &st)
                .expect_err("reached unbounded query comes first")
                .class(),
            dive_eval::ErrorClass::Unsupported
        );
    }
}
