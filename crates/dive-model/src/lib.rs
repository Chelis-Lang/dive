//! `dive-model`: binding a specification to its TLC model configuration
//! (ADR 0013 §8).
//!
//! This is a port of TLC's `tlc2.tool.impl.SpecProcessor` (tlaplus
//! `94d0c50`), with the parts of `Spec` and `Tool` that finish setting up a
//! model. [`bind`] takes the lowered specification and the parsed
//! configuration and does what TLC does before it checks the assumptions, in
//! TLC's order:
//!
//! 1. **Literals** (`processConstants`): a numeral too big for `--int=tlc` or
//!    a decimal stops TLC first.
//! 2. **Names** ([`mod@intern`]): the run's name table is filled in TLC's
//!    encounter order, so that strings and model values order and
//!    fingerprint as in TLC.
//! 3. **Constants and overrides** (`processSpec`): every root constant must be
//!    assigned (`C = v`) or substituted (`C <- D`); `Op = v` and `Op <- D`
//!    replace definitions; `[M]` entries replace definitions of module `M`.
//!    Errors carry TLC's codes ([`Ec`]).
//! 4. **Constant definitions** (`processConstantDefns`): every zero-arity
//!    definition TLC's level bound ([`mod@level`]) finds constant is
//!    evaluated at start-up, module by module, and its value is used from
//!    then on. `Print` output appears here, as in TLC.
//! 5. **Roles** (`processConfig`): invariants, the behavior specification
//!    (`INIT`/`NEXT` or `SPECIFICATION`, split into initial predicates, the
//!    next-state relation and temporal formulas), properties (split into
//!    invariants, implied initial predicates, implied actions and temporal
//!    properties), constraints, `VIEW`.
//! 6. **Actions** (`Tool.getActions`): the next-state relation split into
//!    TLC's actions, with their labels, as `dive_eval::next` actions (with
//!    the default `enumerate` feature).
//!
//! The result is a [`Model`]: an [`dive_eval::Evaluator`] with everything
//! bound, and every role as a [`Pred`] (an expression and the scope TLC
//! evaluates it in). Features dive does not check yet (liveness, `SYMMETRY`,
//! operators it does not implement) are listed in [`Model::unsupported`];
//! the checker refuses such a model by name. Anything this stage cannot do
//! exactly is an [`ErrorKind::Unsupported`] error.
//!
//! # Java overrides and loaded modules
//!
//! Which Java implementation TLC uses for a standard operator depends on the
//! modules loaded: `Integers`' methods replace `Naturals`' when both are
//! loaded. The IR records the active override of every definition, computed
//! from the loaded modules (`dive_ir::std_op`). TLC's `-dumpTrace json`
//! loads `_JsonTrace`, and with it `Integers`, without extending the root
//! module; `dive_sema::Options::extra_root_extendees` reproduces that for
//! oracle comparison (see [`DUMP_TRACE_JSON_MODULES`]).
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use dive_model::{bind, BindOptions};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals, TLC\nCONSTANT N\nVARIABLE x\n\
//!            Max == Print(N, N * 2)\nInit == x = 0\nNext == x < Max /\\ x' = x + 1\n\
//!            Inv == x <= Max\n====\n";
//! let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
//! let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
//! let cfg = dive_config::parse("CONSTANT N = 3\nINIT Init\nNEXT Next\nINVARIANT Inv\n")
//!     .into_config()
//!     .unwrap();
//! let model = bind(ir, &cfg, &BindOptions::tlc()).unwrap();
//! // `Max` is evaluated once, at start-up, as TLC does.
//! assert_eq!(model.output, [dive_model::Message::Print("3  6".to_owned())]);
//! assert_eq!(model.invariants[0].name, "Inv");
//! assert_eq!(model.actions.len(), 1);
//! assert!(model.actions[0].label().starts_with("<Next line 7, col 9 to line 7, col"));
//! ```
#![forbid(unsafe_code)]

#[cfg(feature = "enumerate")]
mod actions;
mod bind;
mod error;
pub mod intern;
pub mod level;
mod model;
mod periodic;
