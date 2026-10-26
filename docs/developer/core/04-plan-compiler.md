# Plan compiler

Status: design. There is no code yet. This document covers strategy S8 from
`01-motivation.md`, and the compiler side of S5 and S6. It supersedes the
evaluation-plan section of ADR 0014 §1.

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

M0 builds:
- a plan compiler covering only the operators used by CoffeeCan1000BeansSafety,
  MCLamportMutex and SimpleRegular, with GameOfLife as a stretch;
- the constants are taken directly from the configuration;
- no INSTANCE, `ENABLED`, error classes beyond abort, or compat order.

The inlining and demand-order analysis are prototyped on these models. Their
effect is measured with an ablation switch that forces every lazy argument to
be a thunk.
