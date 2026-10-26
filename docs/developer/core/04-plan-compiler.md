# Plan compiler

Status: M1 stream 2 in progress. The M0 spike is on `main`
(`crates/dive-core/src/compile.rs`, `plan.rs`, `eval.rs`, `convert.rs`;
results in `m0-results.md`). M1 adds the comparison rules of §5.1
(`compare.rs`), the coverage census of §8 and the fold bound of §3.1 (R9). This document covers strategy
S8 from `01-motivation.md`, and the compiler side of S5 and S6. It supersedes
the evaluation-plan section of ADR 0014 §1.

## 1. Input and output

**Input.** The compiler reads three things:
- `dive_ir::Ir` (`crates/dive-ir/src/lib.rs:96`): an arena of definitions,
  nodes and declarations with resolved ids and slots. Every node carries a
  SANY level and an effects bitset.
- `dive_config::ModelConfig`.
- The startup constants computed by the reference binder (section 2).

**Output.** A `Plan`:
- the compiled roles: initial predicate, actions in TLC order, invariants,
  constraints, action constraints, properties, VIEW and symmetry;
- compact node arrays per compiled body;
- constant pools of persistent handles;
- metadata for the engine: variables read and written by each action, whether
  FP64 is needed, whether runtime names can be created, and refusals.

## 2. Binder handoff

`dive_model::bind` produces a `Model` whose roles close over `dive_eval`
runtime objects (`crates/dive-model/src/model.rs:87`; `Pred` and `Scope` at
`model.rs:14`). It cannot be the new core's input.

The handoff has two parts.

**What is reused from the reference binder.**
- Constant and override processing (`crates/dive-model/src/bind.rs:219`).
- Startup evaluation of constant definitions and ASSUMEs (`bind.rs:617`).
- TLC's level bound (`crates/dive-model/src/level.rs`).
- Their results are `dive_value::Value`s. `dive-api` converts them to handles
  in the permanent region through `dive-core`'s conversion API.
- This keeps the startup errors of constant evaluation, which are part of the
  contract, identical, and it avoids reimplementing about 7k lines.
- Its cost is measured against the 10 ms startup budget. If it is too slow,
  constant evaluation moves into the new core in M2.

**What is built natively.**
- Roles, from the config and `dive_ir`.
- Action splitting, from `dive_ir::actions::split`
  (`crates/dive-ir/src/actions.rs:140`), expanded the way
  `05-successor-generation.md` specifies.
- The plan itself.

## 3. Static passes

The passes run in this order. Each must preserve the semantic contract. Where
a pass would change evaluation order or what gets evaluated, the conditions in
section 4 decide.

1. **Constant folding.** Expressions that are constant-level under TLC's level
   bound, and whose startup value is known, become pool constants. This covers
   overridden definitions (`Op <- D`, `Op = v`) and module-scoped overrides.
   - **Bound (R9).** A fold may visit at most `FOLD_FUEL` (4,096) loop
     elements, and all folds of one compile together at most `FOLD_BUDGET`
     (2^22): the compile-time budget. A fold that runs out of fuel is
     abandoned, and its node and every node containing it stay unfolded, so
     the work happens at run time, where the generator streams it. The plan
     statistics report `fold_abandoned`, `fold_skipped`, `fold_fuel` and
     `fold_ms` (`core.metrics.plan` in the JSON report).
2. **INSTANCE substitution.** `Subst` bodies and `SubstRef` uses
   (`crates/dive-ir/src/expr.rs:342`, `:495`) are inlined by name: each
   reference is replaced by the bound argument expression in the substituting
   context.
   - This is call-by-name, so it evaluates the argument exactly where TLC's
     dynamic lookup would.
   - **Expected gain is uncertain.** ADR 0104's substitution-cache probe
     found that much of the 52.6% substitution cost on EWD998PCal is
     re-evaluation that TLC also performs: primed substitutions are
     re-evaluated in a null context. The reference engine already caches
     unprimed substitutions. The gain from inlining is the removal of the
     dynamic chain lookup and of thunk allocation, not of the evaluations
     themselves. It is measured on EWD998PCal early in M1.
   - **Exception.** The pass keeps the dynamic, cached form (a thunk) where
     TLC's `LazyValue` reuse or the `ENABLED` cutoff is observable. That is
     where a substitution is reachable under `ENABLED`, under prime cutoffs,
     or where the argument has effects. See the `lazy-enabled-cache` corpus
     and `02-semantic-contract.md`.
3. **Operator inlining.** Non-recursive user operators whose bodies are below a
   size threshold are inlined. Each parameter is either substituted (when the
   demand-order analysis allows eager or by-name use) or bound to a thunk.
4. **Slot resolution.**
   - Locals become frame offsets in a flat per-activation frame.
   - Captured variables of `LAMBDA` and of `LET` operators become explicit
     closure slots.
   - State-variable reads become `VarRead(i)` and `PrimedRead(i)`.
5. **Specialization.** The pass picks specialized node kinds for common
   shapes:
   - integer arithmetic and comparison, per integer profile;
   - boolean junctions that return `bool` directly;
   - `x' = e` and `x' \in S`;
   - `UNCHANGED` of variable tuples;
   - `EXCEPT` with constant or simple paths;
   - record field access by constant name, using the shape index when the
     shape is known;
   - function application on interval domains;
   - membership on known set shapes without enumerating them: `a..b`,
     `[D -> S]`, `SUBSET S`, record sets, `Seq(S)` and enumerated sets;
   - `\E` and `\A` over enumerated domains as loops;
   - `Cardinality`, `Len`, `Append`, `Head` and `Tail` on known kinds.
6. **Read sets and purity.** Every compiled body gets the set of state
   variables (primed or unprimed) and constants it reads, plus a purity flag.
   - A body is impure if it reaches `TLCGet`, `TLCSet`, random draws, `Print`
     or I/O.
   - It is cutoff-sensitive if its result depends on `ENABLED` cutoff
     semantics.
   - Memoization uses these flags (`06-memoization.md`). The engine uses the
     per-action write sets for delta hashing.

## 4. Laziness: the demand-order analysis

TLC creates a `LazyValue` at:
- user-operator arguments;
- zero-arity `LET` definitions;
- substitution bindings.

The IR marks these points as `Pass::Lazy`, `Strict` or `Direct`
(`crates/dive-ir/src/expr.rs:53`). Laziness is observable only through effects
and errors: whether an error happens at all, and which error is reported
first. The evidence and fixtures are in `02-semantic-contract.md`.

`Effects::MAY_FAIL` (`crates/dive-ir/src/effects.rs:30`) is set on almost every
expression, so it cannot decide where thunks go. The compiler runs a
demand-order analysis instead. A lazy argument may be evaluated eagerly at the
binding point only when both of these hold:

1. **It is demanded on every path.** It is evaluated on every path through the
   body before the body completes normally, so eager evaluation cannot
   introduce an error TLC would not raise.
2. **It is not reordered past a failure.** No other expression that can raise
   an error, or that has effects, is evaluated between the binding point and
   the argument's first use on any path. So eager evaluation cannot change
   which error is reported first.

The analysis also has these rules:

- An argument that is pure and cannot fail is always evaluated eagerly. Literals,
  bound variables, state reads, and aggregates of those cannot fail.
- An argument used several times with no effects and no cutoff sensitivity may
  be evaluated once and shared. TLC's cache reuse would give the same value.
- Any other argument becomes a thunk in the arena. A thunk holds the plan node,
  its frame, and a cache slot.
  - **Under `--compat=tlc`.** The cache is reused exactly under TLC's
    `getCachedValue` rule, stale reuses included: same predecessor, same
    root-module assignments made when it was forced, equivalent mode
    (`crates/dive-eval/src/env.rs:518-597`; `02-semantic-contract.md` B2).
  - **In the default profile.** The cache is reused only when every input it
    depends on is unchanged (`02-semantic-contract.md` §3a).
  - **Either profile.** Generation stamps on the successor buffer make the
    common case a single comparison.
- Generator `LET` values and substitutions stay uncached, as TLC binds them
  (`02-semantic-contract.md`, section on successor generation).
- Standard-module operators of the value kind evaluate their arguments eagerly,
  left to right, as TLC's `OpValue.eval` does.

The analysis is conservative. When it cannot decide, it keeps the thunk. Its
correctness is checked by shadow comparison (`08-correctness.md`) and by the
laziness fixtures. Under `crates/dive-eval/tests/fixtures/specs/` these
include `EvLazy` and the `next/` fixtures.

## 5. Error classes

Compiled nodes raise typed errors that carry the error-class (EC) code. They
also carry the EvalException-versus-override distinction, which decides exit
code 75 versus 255. The checker's site wrapping, the 2111, 2113 and 2105 rules,
and the call-stack replay all follow `02-semantic-contract.md`.

Message text is not contract. The new core produces clear messages with source
spans. It may reuse the reference engine's text, but it is not required to
reproduce TLC's text.

### 5.1 Comparison errors (`crates/dive-core/src/compare.rs`)

Equality and membership follow the default profile's D-009 rules as the
reference engine applies them (`crates/dive-eval/src/tla_cmp.rs`), with TLC's
errors where TLA+ leaves the answer unspecified (`02-semantic-contract.md`
C1–C3):

- `a = b`, `a # b`, `x' = e` against an assigned `x'`, and `UNCHANGED` against
  an assigned variable raise TLC's error when the operands are of different
  classes (boolean, integer, string, set, function). Inside sets and
  functions, values of different classes are unequal.
- Membership in an enumerated set that misses raises TLC's error when the set
  holds a scalar of another class than the element. The set's sorted order
  groups classes, so its first and last elements decide.
- Membership of a non-integer in a non-empty interval, `Nat` or `Int`, of a
  non-set in `SUBSET S`, and of a non-function in a set of functions,
  records, tuples or sequences raises TLC's error. A function of the wrong
  shape is not a member (D-009).
- `EXCEPT` at a key outside the domain leaves the function unchanged, unless
  the key had to be compared with a domain value of another class.
- **Refusal.** The core builds every set normalized, but TLC normalizes
  lazily, and whether a set that mixes classes (or records with tuples, C2,
  C5) raises an error, and which, depends on when TLC normalizes it. The core
  refuses such a set with `unsupported`, naming the mixture.

The M0 core answered all of these with `FALSE`, so specifications that TLC
stops with an evaluation error passed (`ErWrapSetMember`); see §8.

## 6. Interpreter

The plan is executed by a recursive `match` over compact node arrays: an enum
with indices into the plan's arrays, not boxed trees.

Evidence from ADR 0104 and from tla-mc (`docs/survey/competitive.md`) shows
that dispatch style is not the bottleneck. Continuation-passing and
"invocation session" designs regressed by 10–73% in ADR 0104. The interpreter
therefore stays simple. Its speed comes from:
- the value model (`03-value-model.md`);
- specialization (section 3);
- memoization (`06-memoization.md`);
- the absence of allocation.

Two possible later tiers are explicitly out of scope until M4: a closure tier
and generated Rust. Either would implement the same evaluation interface and be
shadow-compared against this interpreter.

## 7. Next step

M1 stream 2 order: coverage census (§8), comparison errors (§5.1), the fold
bound (R9), laziness (§4), operator breadth in census order, error classes
(§5), then the memoization decision (`06-memoization.md`).

The state of M0, which M1 replaces item by item:

M0 built a compiler for the operators of the four spike models and the
fixtures in `crates/dive-api/tests/fixtures/core-m0`. Measurements are in
`m0-results.md`. Every other construct is refused at compile time, naming it.

- **Binder handoff.** M0 reuses the reference binder's split actions, roles,
  constants, overrides and startup values (`crates/dive-api/src/core_engine.rs`).
  This deviates from §2, which calls for a native split.
- **Inlining.** Every non-recursive operator is inlined. An argument that is a
  literal, a constant, a bound variable or a state read is substituted. A
  state-level argument becomes a thunk slot, and an argument that reads primed
  variables is passed by name. Zero-arity `LET` definitions are thunks.
- **Caching.** A zero-arity top-level definition of state level is evaluated
  at most once per frame activation (for example `sc` in GameOfLife).
- **Recursive operators.** They are compiled once into their own frame, with
  eager arguments. Function constructors are materialized.
- **Folding.** Closed subexpressions are folded by evaluating them at compile
  time, within the R9 bound (§3, pass 1). A folding failure leaves the node
  alone. Filters and `CHOOSE` enumerate a symbolic domain through its cursor
  instead of materializing it first.
- **Errors.** Evaluation errors are reported as `unsupported`. M0 does not
  classify them (§5 is not built).

Next (M1, stream 2):
1. Native action splitting and constant evaluation, to drop the reference
   binder from the handoff.
2. The demand-order analysis of §4, replacing the conservative rule above.
   Lazy function values are needed to remove GameOfLife's materialization.
3. Error classes (§5) and the INSTANCE and `ENABLED` passes.

## 8. Coverage

The census that orders stream 2's work. Raw outputs are kept outside the
repository; the commands are below.

**Method.**
- Small tier: `DIVE_ENGINE=core dive-conformance run --impl dive --corpus
  examples --corpus probes --corpus validation-smoke --corpus
  configured-temporal-operators --tier small --workers 1 --no-sany
  --tlc-budgets --budget-cap 900` (156 models).
- Latent refusals: each model refused for an engine feature (temporal
  properties, `SYMMETRY`, `VIEW`, `ALIAS`, `ACTION_CONSTRAINT`,
  `POSTCONDITION`) is rerun with those configuration sections removed, core
  against the reference engine. This shows what the compiler refuses behind
  the engine refusal.
- Fixtures: every `.cfg` under `crates/*/tests/fixtures/` except the
  configuration-parser fixtures (1,269 models), core against the reference
  engine (shadow comparison, `08-correctness.md`), 20 s each.

**Baseline, M0 at `866c0978`.**

Small tier: 24 agree, 2 expected divergences, 4 disagree, 126 unsupported.
The four disagreements are missing counterexamples (traces are stream 3);
no count disagreed.

| Unsupported reason (small tier) | Models |
|---|---|
| temporal properties (engine, M3) | 70 |
| model values (values, stream 1) | 16 |
| `SYMMETRY` (engine, M2) | 11 |
| value override of a definition (a model value) | 5 |
| split argument that is not constant (binder handoff) | 5 |
| `VIEW` or `ALIAS` (engine, M2) | 4 |
| INSTANCE substitution | 3 |
| `TLC!RandomElement` (M2) | 3 |
| `IOUtils` I/O (M2) | 2 |
| standard operator not implemented | 2 |
| false or failing `ASSUME` (handoff) | 1 |
| evaluation error, unclassified (§5) | 1 |
| recursive `LET` operator | 1 |
| `ACTION_CONSTRAINT` (engine) | 1 |
| `TLCGet`/`TLCSet` (M2) | 1 |

Latent refusals (126 models with the engine sections removed): 41 agree with
the reference engine, none disagree, 85 are refused:

| Latent refusal | Models |
|---|---|
| model values (values, stream 1) | 40 |
| value override of a definition (a model value) | 12 |
| split argument that is not constant (handoff) | 10 |
| INSTANCE substitution | 6 |
| standard operator not implemented (`Bitwise`, `Bags`, `Json`, `TLCExt`, `UndirectedGraphs`) | 6 |
| constant operator | 2 |
| `TLC!RandomElement`, `IOUtils` (M2) | 4 |
| other (recursive `LET` operator, `SelectSeq` with an operator argument, `[A]_v` as a value, `TLCGet`, failing `ASSUME`) | 5 |

Fixtures: 284 agree with the reference engine, 963 are refused, 7 differ,
15 are skipped (no module, or the reference refuses). The refusals are led by
false `ASSUME`s (210), temporal properties (about 400), unclassified
evaluation errors (about 110, §5), `VIEW`/`ALIAS` (26), `PrintT` (20),
`SYMMETRY` (28), INSTANCE (15) and `ENABLED` (14).

**Disagreements found, and their owners.**
- *Comparison errors (fixed, §5.1).* `ErWrapSetMember` passed in the core and
  is an evaluation error in TLC. Thirty probe specifications of comparisons
  and memberships between classes showed 22 wrong verdicts. All now raise the
  error, or are refused.
- *Engine (stream 3).* `DfidInitialOnly` (no next-state action) is a deadlock
  in the core and TLC's 2115 model error in the reference.
  `SimExcludedInitial` (the only initial state fails the constraint) reports
  depth 0, the reference depth 1. Four runs at the 20 s time limit end as
  `resource-limit` (exit 3), the reference as a D-026 `stopped` (exit 5).

**Priorities drawn from the census.** Model values (stream 1) gate the
largest group. In the compiler: the binder handoff of non-constant split
arguments, INSTANCE, then evaluation-error classes, which hide most fixture
coverage. Laziness comes first among them, because classifying errors is
sound only once the core raises no error TLC would not.

## 9. Interface

Changes made by other streams to the plan and compiler, recorded here as
`10-plan.md` §3 asks.

- **Stream 3, `core/engine-getnext`.**
  - `plan.rs`: new generator nodes `AssignInRaw`, `StepAngle` and
    `FcnAction`; `Unchanged` and `StepBox` hold `Unch` items (`Var` with its
    site, or `Verify(P)`); the `Raw` tree; `Plan::action_constraints`.
  - `compile.rs`: `Handoff::action_constraints`; the generator arms of `g`
    for `\in`, `\subseteq`, `UNCHANGED`, `[A]_v`, `<<A>>_v` and `f[a]`; the
    value forms of `[A]_v` and `<<A>>_v` in `p`. The new helpers live in
    `compile_gen.rs`, a child module of `compile`.
  - `eval.rs`: `Machine::warnings`.

