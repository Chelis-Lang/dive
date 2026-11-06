# Plan compiler

Status: M1 stream 2 in progress. The M0 spike is on `main`
(`crates/dive-core/src/compile.rs`, `plan.rs`, `eval.rs`, `convert.rs`;
results in `m0-results.md`). M1 adds the comparison rules of §5.1
(`compare.rs`), the coverage census of §8, the fold bound of §3 (R9), the
laziness of values (§4.1), `INSTANCE` substitution (§3, pass 2), Bags, typed
TLC error classes (§5.2), model values (§5.3), operator arguments (§5.4), lazy unions (§4.2),
GameOfLife's lookups (§6.1) and the memo decision
(`06-memoization.md` §6).
This document covers strategy S8 from `01-motivation.md`, and the compiler side of S5 and S6. It supersedes
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
  constraints, action constraints, properties, VIEW and symmetry. The
  binder's initial predicates are conjuncts, as in TLC and the reference
  engine's `init_states`. For example, `Spec == Init /\ y \in S /\
  [][Next]_v` has two. The plan conjoins them into one initial action
  (`init_action`), named after the first. Until that fix, the core ran each
  as a separate initial action, and reported an incomplete initial state
  where TLC reports success (fixture `InitConj`, RoSpec);
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
  A false or failing ASSUME ends a core run with the reference engine's
  verdict: `AssumptionFalse` (TLC 2104) or `classify::assumption_error`
  (2105, or the error's own class), exit 10, before any core work. The
  fixtures `AsFalse` and `AsErr` match the pinned TLC. A run that printed
  output at startup is refused, because the core does not report startup
  output.
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

### 2.1 Roles inside a `LET` (built)

TLC's `getActions` splits through a `LET` (DieHarder's `JugToJug(j, k) ==
LET amountPoured == ... IN contents' = ...`), and binds the `LET` values
lazily in the action's context.
- **Before.** The handoff read every slot of the role's frame, and evaluated
  each lazy one as a constant. A `LET` value that reads the state therefore
  refused the model: "a split argument that is not constant". A constant one
  was evaluated eagerly, which TLC never does.
- **Now.** The handoff skips `LET` slots (`is_let_slot` in
  `crates/dive-api/src/core_engine.rs`). `bind_role_lets` binds them as
  `Sb::LetVal`, a thunk forced at most once per evaluation of the action,
  since the action's frame starts empty.

Fixture `RoleLet` (54 distinct, 649 generated, depth 7, as TLC) has a
state-reading `LET` value and an unused failing one. MCDieHarder (violation,
14/73/7) and kvstore (2641/28585/9) now agree with the reference engine.

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
   - **Built (M1).** The compiler keeps a dynamic substitution stack
     (`SubstEntry` in `compile.rs`). It is a stack, not a rewrite, so the
     innermost active substitution wins, as in TLC's dynamic lookup.
     - **Binding.** `Expr::Subst` binds its substitutions as operator
       arguments are bound. Each value is compiled outside the substitution,
       then aliased when it is trivial, made a per-activation thunk when it
       reads no primed variable, or else passed by name.
     - **`SubstRef`.** It reads the innermost binding, or else falls back to
       the root variable or the configured constant. `x'` with `x <- e` is
       `e'`, compiled by name.
     - **Generator.** It sees through substitutions for assignments
       (`getPrimedVar`: `v' = e` with `v <- x` assigns `x`), for `UNCHANGED`,
       and for substituted names used as actions (by name, B5).
     - **Operators.** An operator substitution `F <- G`, with `G` a top-level
       or standard operator, is applied in the current context, as TLC does.
     - **Compiled definition identity (#634).** A zero-arity nonrecursive
       definition's cached body is keyed by definition, plain/primed mode and
       the active substitution context, in ordinary and step predicates.
       SANY shares original definitions across nested instances, so the same
       `DefId` can compile through different actuals. Each immutable substitution
       entry has a unique id; the last id identifies its entire ordered active
       prefix, since entries are only appended fresh, truncated or restored.
       Empty context has its own key. `outside` therefore selects the outer
       prefix's identity, rather than reusing an inner instance's body.
       Existing free-slot/effect/candidate guards still decide eligibility.
     - **Split roles.** For actions and checks that the reference binder
       found inside an instantiated definition (`Init == I!Init`), the
       handoff passes the substitutions active there (`Role::substs`, from
       `scope_subst_slot`). They are bound by name. Values evaluated under
       substitutions of their own are refused.
     - **`ENABLED` under substitutions** admits constant/operator parameters
       and identity mappings of root variables (§5.5). Hidden or renamed
       variables remain refused, keeping the unimplemented B2/B4 cache
       rules unreachable in both profiles.
     - **Evidence.** The fixture `crates/dive-api/tests/fixtures/core-m0/Inst.tla`
       gives 32 distinct, 65 generated, depth 8 with the pinned TLC, the
       reference engine and the core. So do the probes run against the pinned
       TLC (raw outputs outside the repository):
       - a parametrized instance;
       - a nested instance (`M!I!Init`);
       - `v <- x + y` under a prime;
       - an operator substitution;
       - a lazy substituted argument.

       Two of the probes found reference defect #515, a spurious 2280 on
       `2 * v` with `v` substituted. Its minimal corpus now agrees on both
       engines. Naturals' dummy multiplication body gives SANY a constant
       level even when native arguments read state. The binder's configured
       level bound already skips startup precomputation correctly; the
       reference evaluator's opportunistic empty-state cache now separately
       proves state independence instead of trusting SANY's level alone.
       A non-evaluating walk follows real native arguments and configured
       targets, charging at most 4,096 nodes, references and child edges.
       Constructor-bound value slots remain eligible when the walk checks
       their domains in the same definition frame; this preserves deferred
       set identity and retained materialization history. Parameters, outer
       captures and LET thunks remain unknown. State, unknown dynamic inputs,
       recursion and exhausted proof work disable only that cache. The result is kept once per definition
       in private evaluator metadata; no source body is evaluated by the
       proof. Explicit successful startup values and configuration redirect
       semantics are unchanged. The consumed controls include constant
       instance startup values, unused failing/effectful actuals, output,
       multiplication argument-error order and a fresh independently checked
       error word. Nested scopes expose a separate existing core/auto false
       alarm (#634), so that case earns reference agreement only.
       MCChangRoberts now runs in the core with TLC's counts.
3. **Operator inlining.** Non-recursive user operators whose bodies are below a
   size threshold are inlined. Each parameter is either substituted (when the
   demand-order analysis allows eager or by-name use) or bound to a thunk.
4. **Slot resolution.**
   - Locals become frame offsets in a flat per-activation frame.
   - Captured variables of `LAMBDA` and of `LET` operators become explicit
     closure slots.
   - A LET-bound recursive function uses its owning definition's frame layout.
     Its self/argument slots are rebound; other slots become lazy captures of
     the actual enclosing activation. Compilation distinguishes that activation,
     plain/primed mode, Init context and active substitutions. Calls pass captures
     through the existing lambda-lifting mechanism; no caller frame offset is
     reused in the callee frame.
     External substitution expressions are compiled inside that callee frame;
     proved constants can be reused. Non-lexical captures and effectful or
     recursive substitutions refuse rather than borrowing caller thunk slots.
     Every compiled function records this runtime-frame boundary, including
     top-level recursive functions with no lexical parent.
     Primed applications compile the domain and body against the candidate
     state; their bound argument stays a local slot. Self calls must use the
     definition's compilation mode. Primed enclosing-local captures, mixed-mode
     self calls and effectful primed functions remain named refusals.
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
   - **Built (M1): the effect bit.** `Info::effect` marks a node that
     reaches `Print` or `PrintT` (`stdlib::effectful`). Such a node is never
     folded, never a memo site, never a cached state-level definition, and
     never a skippable invariant conjunct, since TLC prints on every
     evaluation. An effect inside a function constructor, filter, set map,
     speculative evaluation or library operator argument is refused: there
     TLC's evaluation count and order follow its value representation, not
     the expression. `Plan::effects` makes the engine run one worker, so
     the output is in TLC's order, and refuse a run that does not succeed
     (TLC prints again while it rebuilds the trace) or that warns. Fixtures
     `PrOps` (printed output equal to TLC's, at 1 and 4 workers), `PrViol`
     and `PrLazy` (refused).

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

### 4.1 Lazy values (built)

Operator arguments and `LET` values are already thunks (M0). M1 adds TLC's
laziness of values: TLC builds a filter, a function constructor or a set
operation lazily unless its operands are `Reducible`, and applies a function
lazily in `f[a]`. Laziness is observable only through errors, because the
core has no effects yet (B6), so the rules below are about which errors
happen.

**Reducibility.** The compiler records for every node whether TLC's value is
`Reducible` (a set enumeration or an interval;
`crates/dive-eval/src/core.rs`, `is_reducible`): `Yes`, `No` or `Maybe`
(`Info::red` in `compile.rs`). The value is `Yes` for an enumeration, an
interval, a set map, a filter or set operation over `Yes` operands, and a
constant of those kinds. It is `No` for `Nat`, `Int`, `Seq(S)`, `SUBSET`,
products, function sets, record sets, and filters or set operations over `No`
operands. Every other node, such as a state variable, a bound variable or a
call, is `Maybe`. Folding keeps the expression's value.

**Rules.**
- **Eager where TLC is eager.** A filter, a function constructor or a set
  operation (`\cup`, `\cap`: both operands; `\`: the left operand) over
  `Yes` operands is built eagerly, as TLC builds it.
- **Speculative elsewhere.** Otherwise the core still builds it eagerly, under
  `Node::Spec`. An evaluation error inside becomes `unsupported`, naming the
  laziness, because TLC might never raise it. The plan statistic
  `speculative` counts these nodes.
- **Membership.** `x \in S`, where `S` is a filter or a set operation that
  TLC certainly keeps lazy (`No`), is decided structurally, as TLC's
  `SetPredValue`, `SetCupValue`, `SetCapValue` and `SetDiffValue` decide it:
  the domain first, then the predicate, short-circuiting left to right
  (`Node::InLazy`). This is exact, so it is not speculative. It also makes
  membership in a filter over `Nat` work.
- **Application.** TLC evaluates `f` in `f[a]` with `KEEP_LAZY`. So a function
  constructor reached directly, through inlined operators, or through a
  top-level definition stays lazy. So does one over a `No` domain. One
  reached through a `LET` value or an operator argument (TLC's `LazyValue`)
  over reducible domains is built eagerly (probe `LzApplyDef` below). A lazy
  constructor with one bound variable is applied without being built
  (`Node::Beta`): its domain, then the argument, a domain check, then the body
  for that argument alone (`FcnLambdaValue.apply`). The plan statistic `beta`
  counts these. A lazy one with several bound variables is built
  speculatively.
- **`DOMAIN`.** `DOMAIN f` of a constructor over a `No` domain is its domain,
  without building it (`FcnLambdaValue.getDomain`). `EXCEPT` bases and
  `DOMAIN` are not `KEEP_LAZY` positions (`crates/dive-eval/src/eval.rs`,
  `OPCODE_fa` only).
- **Recursive functions.** `f[x \in S] == e` (top level, one bound variable)
  is compiled as a function of its argument: `Node::DomCheck` checks that the
  argument is in `S`, then evaluates `e`. `f[a]`, outside or inside `e`, is a
  call. Other uses of `f` as a value are refused, and so are `LET`-defined
  recursive functions.
- **Recursive operators.** TLC passes their arguments lazily. The core
  evaluates them at the call, so a non-trivial argument is speculative.
- **Nested recursive definitions.** A recursive `LET` operator is
  lambda-lifted. Its references to enclosing locals become captures, passed by
  every call after the arguments and by the recursive calls inside it
  (`Lift`, `Fun::caps`). A captured thunk is forced at the call, so it is
  speculative. Mutually recursive nested definitions, and captures under a
  prime, are refused.
- **Startup values.** TLC's startup value of a constant definition may be
  lazy, for example a filter over `Nat`. When it cannot be converted, the core
  compiles the definition's body instead. Configuration overrides
  (`Handoff::overrides`) are never recompiled.

**Not yet.** These need further demand-order or hidden-query lifetime support:
- the demand-order analysis proper;
- the B2 and B4 cache rules, which need hidden variables under an
  `INSTANCE` query (still refused, §5.5, so D-027 is not yet observable);
- streaming a lazy filter as a quantifier or `CHOOSE` domain with
  short-circuit.

A speculative region therefore never yields a wrong verdict, only
`unsupported`. Error classification (§5) can rely on every remaining
evaluation error being one TLC raises.

**Probes.** These are run against the reference engine, and the raw outputs
are kept outside the repository:
- `LzApplyRed`, `LzApplyDef`, `LzDomain`, `LzDomOut`, `LzExcept`;
- `LzNatApply`, `LzNatOut`, `LzFilterOk`, `LzFilterErr`;
- `LzLetRec`, `LzLetRecErr`, `LzNestRec`, `LzRecOut`.

Every one agrees with the reference engine, or is refused. The fixture
`crates/dive-api/tests/fixtures/core-m0/Lazy.tla` covers the successful paths
in the test suite.

### 4.2 Unions with an infinite side (built)

TLC keeps `A \cup B` (`SetCupValue`) and `UNION S` (`UnionValue`) lazy.
Membership tests each side, or each member, in turn. Enumerating one with an
infinite side fails.
- **Before.** The core refused `A \cup B` with an infinite side ("union of
  an infinite set"), and enumerated `UNION` members, which fails on an
  infinite member.
- **Now.** Both build the heap's lazy `Kind::Union` node (`Heap::union`)
  when some side or member cannot be enumerated. The membership arm already
  follows `SetCupValue.member`. Finite unions are still built eagerly.
- **Errors.** An error raised while normalizing or testing one, such as
  `UNION {Nat, {NULL}}`, is reproduced with the reference evaluator, as
  every core error is (`crates/dive-api/src/core_ending.rs`).

Fixture `InfCup` (18 distinct, 100 generated, depth 3, as TLC) covers:
- `Int \cup {NULL}`;
- `UNION {[D -> Int \cup {NULL}]}`;
- `Seq(Int \cup {NULL})`;
- a finite `UNION` joined with `Nat`.

MCDisruptor_MPMC (112929 distinct, 422781 generated, depth 81) agrees
with the reference engine once §2.1 is in as well.

**`UNION {S}`.** When `S` is a set constructor (`[D -> R]`, `[a : R]`,
`SUBSET R` or `A \X B`), `UNION {S}` compiles as `S`. TLC's `UnionValue`
tests membership in each member, so this is exact. Other operands keep the
check that the member is a set. Compiling the set enumeration would
normalize `S` as its element. For
RingBuffer's `UNION { [0 .. LastIndex -> SUBSET Readers] }` that enumerates
and sorts 4096 functions in every state, because `Readers` is an instance
substitution and is not folded. MCDisruptor_MPMC at one worker, user time:
core 35.3 s before and 0.9 s after; the reference engine takes 3.8 s.
Fixture `UnionOne`: TLC reports `Inv` violated, 8/11/2.

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
`crates/dive-api/tests/core_kinds.rs` checks 38 comparisons across kinds
against the reference engine: each one gets the reference result or a
refusal, and only the mixed-class set may be refused where the reference
engine answers.

### 5.2 Typed classes (built)

The minimal `next-receiver-lifetime/EmptyLazyQuery` control records the #680
Next-side function demand before implementation: Init emits `es={}` and
`step=0`; Next queries `[i \in 1..2 |-> 1 \div 0] \notin es` before any
successor. The clean pinned TLC stream reports underlying division-by-zero 2179,
a single original source row and a diagnostic fingerprint exception 2147 with
nested 2179; physical exit is 75, distinct/generated/depth are 1 and queue is 0.
It is not a candidate-side error or an empty-membership shortcut. The value's
actual FunctionMaterialize catch and newly constructed diagnostic source must
establish the wrapper. Cached/stored receivers and later/multiple source
lifetimes gain no authority from the same error number or printed text. A constant-level function definition
is not itself a cached value: the `DefinedFunction` control also fails during
construction before it could cache a value, and its actual newly constructed
diagnostic receiver reproduces pinned 2147/75. Source syntax alone establishes
neither positive nor negative receiver lifetime.

The hash-bound clean consumer inputs live under
`crates/dive-conformance/tests/fixtures/next-receiver-lifetime/`, recorded with TLC `94d0c5070852b37b801bb8998c436383ab7ee37b`, jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`,
JRE 17.0.20.1+1, seed 0 and fingerprint 0. Source and configuration live under
`crates/dive-api/tests/fixtures/next-receiver-lifetime/`; hash-bound actual
consumer tests retain these original observations, without generated goldens.
The bounded admission reconstructs exactly one exhausted Init emission, the
first source and no preceding complete/partial Next candidate. Ordinary and
diagnostic Next execute within the same fresh terminal runtime; a separate
prefix runtime supplies no caches. Later/multiple/effectful and prior-candidate
controls retain their original unverified result, cause, output and counts.
The producer and actual source-bound consumer each require this proof. Existing
Next endpoint and report versions retain their meanings.

Initial-invariant runtime contexts without an actual sourced diagnostic receiver
do not inherit ordinary successor replay verification (#685). The shared
`init_error` path retains the original cause and recorded-state 2102 context,
observed 2103 wrapper and physical 75, but marks that class unverified. Reference
and core suppress the anonymous formal word and keep actual stopping counts;
they do not manufacture a named or auxiliary proof. The pinned `InitialTool`
and `InitialOverride` negatives have only 2102/context 2103 ERROR messages and
remain Unclassified. Their former `verified=true` expectations were documented
bugs. Actual initial arithmetic and the separately proved 2147/2102 receiver
route retain their contracts. This honesty repair adds no oracle agreement or
new trace/report version.

The focused consumer test reads hash-bound clean stdout fixtures under
`crates/dive-conformance/tests/fixtures/initial-context-unclassified/`. Both were
recorded with pinned TLC `94d0c5070852b37b801bb8998c436383ab7ee37b` (jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`),
JRE 17.0.20.1+1, one worker, seed 19, fingerprint 0 and no diagnostic trace dump.
Both actual physical exits are 75. `InitialTool` retains GENERAL/1000 as its
underlying cause; `InitialOverride` retains JavaMethodOverride/2154. Canonical
classification stays Unclassified in both public and parser-owned paths; native
reports refuse classification with and without a source model.

An evaluation error carries `Option<TlcClass>` (`crates/dive-core/src/lib.rs`):
TLC's `EC` code and whether TLC raises it as an `EvalException`.
- **`None`:** an unclassified error. The core has not established that TLC
  raises this error here, or with which class. The engine must refuse it
  (`unsupported`, naming it), never report it as a verdict.
- **`Some`:** classified. The error is raised exactly where TLC raises it,
  with TLC's class. The engine applies the site rules: 2111, 2113,
  `GENERAL`'s prefix and the replay. Messages keep TLC's beginning where
  the replay rule reads it ("In applying the function\n", "...of
  form\nCHOOSE").

| Error | Class |
|---|---|
| `\div` by 0 | 2179 `EvalException` |
| `%` by a non-positive number; `^` with a negative exponent | 2169 `EvalException` |
| `0^0` | 2180 `EvalException` |
| `Head`/`Tail` of `<<>>` | 2184 `EvalException` |
| `Len` of a non-sequence | 2283 `EvalException` |
| `Cardinality` of a non-enumerable value | 2181 `EvalException` |
| a non-integer `\in Nat`/`Int` | 2158 `EvalException` |
| comparisons between classes (§5.1) | `GENERAL` |
| a lazy or recursive function applied outside its domain | `GENERAL` |
| `f[a]` outside the domain, with no class clash | `GENERAL` |
| `CHOOSE` with no witness | `GENERAL` |
| a non-integer in a non-empty interval | `GENERAL` |

Every other evaluation error is unclassified. The arithmetic classes, and
TLC mode's 32-bit overflow (2178 `EvalException`), are raised in
`crates/dive-core/src/int.rs`.

**Defect fixed.** The M0 core computed `0^0` as 1 and reported success.
TLC raises "0^0 is undefined." On fixture `Pow0`, TLC, the reference engine
and now the core report the 2180 evaluation error after 1 distinct state and
1 generated.

**The engine's part.** Stream 3's error endings (`crates/dive-api/src/core_ending.rs`,
`07-engine.md` §5) reproduce each failing action or check with the reference
evaluator, and classify that evaluator's error with `dive_check::classify`.
The core's class is not yet read there. Two uses are planned:
- a cross-check: the two classes must agree, or the run is refused;
- dropping the reproduction for classified errors, once the classes cover
  the fixtures.

### 5.3 Model values (built)

A model value is a scalar handle (tag 3, `03-value-model.md` §11.1): the
name's id in the string table, distinct from the string with the same text.
Its rules follow TLC's `ModelValue`, as the reference engine ports them
(`crates/dive-value/src/tlc.rs`, `crates/dive-eval/src/tla_cmp.rs`):
- **Order.** Model values sort first, by name (D2), before booleans.
- **Untyped.** An untyped model value is unequal to every other value,
  without error. It is in no interval, `Nat`, `Int`, sequence, function,
  record or tuple set (`FALSE`).
- **Typed.** A model value is typed when its name has more than two
  characters and the second is `_` (`t_a`). It compares with model values of
  its own type and with untyped ones. Any other comparison or membership is
  TLC's `GENERAL` error.
- **Refusals.** Sets that mix a typed model value with values it cannot be
  compared with are refused (C1, C2), as are memberships whose error depends
  on TLC's search order (D3).
- **Value overrides.** `Op = v` for a definition the binder does not
  precompute (`NoVal = NoVal`) reaches the core through
  `Evaluator::value_override`.

Probes against the pinned TLC (`t_a = 1`, `t_a \in Nat`, `t_a \in 1..3`,
`t_a = s_b`, `t_a \in [{1} -> {1}]`, `u = 1`, `u \in [{1} -> {1}]`,
`t_a \in {u, t_b}`, `t_a < 1`) agree with the reference engine and TLC.
Two cases are refused: `t_a \in {1, 2}`, and `1 \in {t_a, s_b}`.
`<<t_a>> = <<1>>` is `FALSE` in the default profile, as in the reference
engine (D-009); TLC raises an error. Fixture `Mv`: 31 distinct, 35
generated, depth 6, as TLC.

**Defect fixed with it.** The empty function had two forms (`Kind::Fcn` with
no keys from `fcn_sorted`, and the empty tuple). So `[k \in {} |-> 1] =
<<>>` was `FALSE`, and `Len` of the first failed. `fcn_sorted` now builds the
empty tuple (fixture `Emp`; MCMajority).

### 5.4 Operator arguments and constant operators (built)

TLC passes an operator argument as a closure over the caller's context, and
applies it there. The compiler reproduces this by inlining.
- **Binding.** A parameter bound to an operator argument is an `Sb::OpArg`:
  the operator, and the environment it was passed in. `resolve_param` follows
  parameters that are passed on (`Through(F, v) == Twice(F, v)`).
- **Application.** The argument is applied the same way in expressions
  (`apply_param`) and in actions (`g_apply_op`). The call's arguments are
  compiled at the application site. A `LAMBDA` or `LET` operator's body is
  compiled in the environment where it was passed, so it reads the caller's
  locals. A top-level operator or a std operator is applied as usual.
- **Constant operators.** `CONSTANT F(_)` with `F <- D` in the
  configuration applies `D`.
- **Declarations an instance substitutes.** These are followed through
  nested instances by `subst_op` (`INSTANCE M WITH G <- F`, where `F` is
  itself substituted). Each link is looked up in the context outside its own
  substitution.
- **Refused:**
  - an operator parameter of a recursive operator;
  - a recursive `LET` operator passed as an argument;
  - an `INSTANCE`-substituted operator passed as an argument;
  - an enclosing operator parameter read inside a lifted recursive `LET`;
  - built-ins passed as arguments (`Op(\cup)`);
  - std operators applied to operator arguments (`FoldSet(LAMBDA ...)`),
    which belong to the stdlib (stream 1).

**For the stdlib.** `Compiler::op_fn(arg, env, arity)` compiles an operator
argument as a function of `arity` fresh frame slots: the slots, and the body
with the operator's parameters bound to them. The operator is resolved as
`apply_param` resolves it. It is the entry point for the standard modules'
higher-order operators (`SelectSeq`, `FoldSet`, ...; stream 1's
`Node::LibHo`). That node sets the slots and evaluates the body for each
element, and treats the slots as bound in `info_of`, as a binder does. Std
and built-in operators passed as the argument (`FoldSet(+, 0, S)`) are
refused for now. `Node::LibHo` is built (§9); `stdlib::op_arity` says which
arguments are operators and at what arity.

**`SelectSeq` under prime.** `lib_ho` admits only
`Sequences!SelectSeq` in `Mode::Primed`, and still compiles its sequence
argument in that mode. Its callback continues through `op_fn` in plain mode,
but is admitted only when the compiled body's `Info` has no unprimed or primed
state reads, effects, unknown calls, or free slots other than the callback's
formal parameters. Resolved constants and locally bound expressions are
allowed. Configuration redirects and `INSTANCE` substitutions are resolved
before this check: their actual state reads and effects therefore contribute
to the same metadata. A lexical capture contributes its free slot. Unknown
calls remain conservatively refused.

For this subset, the callback receives only each element of the successor's
sequence; changing the surrounding state context cannot change its result or
error. No closure context is moved or made eager. The existing `SelectSeq`
runtime still checks its sequence first, applies the callback in sequence
order, and never applies it to an empty sequence. Other higher-order operators
under prime and state-dependent callbacks retain their refusals; free/double
primes still undergo the existing frontend/compiler checks.

`core_primed_selectseq.rs` consumes the compact official-harness facts in
`crates/dive-api/tests/fixtures/core-primed-selectseq/oracle.json`: direct,
configured/formal/LET and EWD-shaped refinement positives, a validated violated
step, demand/error-order controls, and state/capture/effect/prime boundaries.
The adjacent README records the one unclassified oracle negative and the
separate final-expansion word obligation; neither earns agreement credit.
The original pinned `EWD998Chan` config enters core search in both profiles
under a one-second, one-GiB budget and then stops. This establishes admission,
not a completed model verdict, capacity or performance result.

**Fixtures.** `OpArg` (6 distinct, 7 generated, depth 4) and `OpSub` (6/7/5)
match the pinned TLC.

**Models.** These models now agree with the reference engine, properties
stripped:
- CigaretteSmokers: 6/15/2;
- MCInternalMemory: 4408/21400/10;
- TLCSailfish1: 109604/314144/16.

### 5.5 `ENABLED` (built)

`ENABLED A` compiles `A` as an action in the current frame (`Compiler::enabled`, `Node::Enabled`). At run time `Machine::enabled` searches it for one successor from the current state, as TLC's `Tool.enabled` does:
- **Search.** `A` runs with a fresh successor buffer, and the search stops at the first successor, complete or not. TLC returns its partial state function there. So `ENABLED (x' = 1)` is true in a model with other variables.
- **Enclosing generation.** The buffer and the continuation stack of an enclosing generation (`ENABLED` in an action's guard) are saved and restored.
- **Plan facts.** The node reads every variable, so it is never folded, skipped by the parent-delta skip or memoized on a partial key. Its free slots are those the action's value nodes read from outside it (`Compiler::g_free`).
- **Instances.** Constant and operator parameters are supported when every
  variable substitution resolves to the root variable with the same name.
  This first subset excludes effects and recursive/nested query calls.
  `Compiler::enabled_substs` proves that identity through the captured
  substitution scopes using the generator's variable-target resolution. TLC's
  branch cutoff cannot change such a variable lookup. Constants remain visible
  across the cutoff, and existing lazy compilation preserves their demand order.
- **Refused:**
  - Hidden variables and nonidentity variable substitutions inside an instance.
    TLC's `Context.branch` hides outer substitutions from primed lookups
    (`02-semantic-contract.md` B3, open question 8). Its lazy cache also reuses
    stale values across hidden variables (B2, corpus `lazy-enabled-cache`; B4,
    `lazy-query-lifetimes`). All instance controls in those corpora and in
    `lazy-enabled-capture` remain refused in both profiles.
  - Instance queries in an initial predicate: the current query buffer requires
    an established source state. This is a static refusal, allowing `auto` to
    select the reference engine before search.
  - State-dependent configuration operator bindings or definition redirects:
    configuration overrides can bypass the source-level constant constraint.
    This is conservative even for an unrelated state-dependent override.
  - Effects and recursive/nested calls in an instance query. The generator
    traversal checks every referenced value node's `effect` and `calls` facts;
    unknown call effects are not treated as pure.
  - A primed `ENABLED`.
- **Lazy arguments.** Operator arguments that read primed variables are passed by name, as everywhere in the core. TLC's root-module `LazyValue` reuse gives the same answer: corpus `root_remove_two` succeeds with 1 distinct state in both profiles, as with TLC.

**Instance evidence.** `core_enabled_instance` runs seven source fixtures in
both profiles at 1, 4 and 8 workers, under five eager/memo configurations
(210 cells). The pinned TLC jar
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`
with seed 0, fingerprint 0, one worker and final liveness checking gives:

| Fixture | Result | Distinct/generated/depth |
| --- | --- | --- |
| `EnabConstInstance`, `Lazy`, `Chain`, `Op` suffixes | success, exit 0 | 4/5/4 |
| `EnabConstInstanceGuard` | success, exit 0 | 3/4/3 |
| `EnabConstInstanceBad` | `Inv`, code 2110, exit 12 | 2/2/2 |
| `EnabConstInstanceError` | code 2111, underlying 2179, exit 76 | 2/2/2 |

The lazy case leaves `1 \div 0` undemanded; the error case demands it only
at the second state. Independent trace validation accepts both failing words
and rejects forged state/error endings. The test retains 33 existing hidden
instance controls plus nonidentity and initial-context controls (70 refusal
cells). Seven rejected source controls additionally pin the frontend boundary:
constant and operator actuals that capture root state, primed state or hidden
state fail SANY's level-zero requirement (TLC code 3002, exit 150). Forwarding
through an instance formal retains the constraint at its application. Both
engines and both profiles reject all seven (28 cells). Configuration operator
overrides can bypass SANY's source-level check.
State-dependent configured operators/definition redirects are therefore refused
for instance queries until their captures are proved. Effects and recursive or
nested calls in an instance query are also refused, independently of the
parameter's declaration kind.

Nine more source controls pin the configured/effect/call refusals at 1, 4
and 8 workers in both profiles (54 cells), including static auto fallback with
matching output. Pinned TLC succeeds at 4/5/4 for configured root/hidden state
and prime captures, a definition redirect and a recursive operator; guard-based
`PrintT`, `RandomElement` and `TLCSet` controls succeed at 3/4/3. These cases
remain deliberately refused until their broader contexts are proved.

Source mutation controls detect restoring the blanket refusal, removing the
identity proof, removing the initial-predicate refusal, and separately removing
the configured-capture, effect and unknown-call checks. No B2/B4 default change
or new divergence is implemented by this subset.

**Fixture.** `Enab` (TLC: `Inv` violated, 216 distinct, 672 generated, depth 12) covers:
- guards;
- partial assignment;
- a conflicting assignment;
- `\E` inside the action;
- `[A]_v` and `<<A>>_v`;
- operator arguments from a binder;
- `ENABLED` in an action's guard.

`EnabStep` (TLC: success, 3/4/3) puts `ENABLED` in an implied action and
an action constraint. A primed definition inside it must read the
`ENABLED` action's successor, never the step's, so a step predicate's
cache of primed bodies (#579) must not reach inside `ENABLED`.

**Cache boundary defect (#588, 2026-10-28).** The original `EnabStep`
uses `NX == x'`, which is too trivial to enter `step_cached`. Replacing it
with `NX == x' + 1` and adjusting the checks to `NX < 4` and `NX = 8`
exposed an enclosing step thunk reused inside the query: pinned TLC and the
reference engine succeed (3 distinct, 4 generated, depth 3), while the core
reported an action-property violation in both profiles. **Fixed:** the query
compiles with a fresh `def_cache`, restoring the enclosing cache afterward,
as well as generator compilation with `step` off. `EnabStepCache` retains
that holding control; `EnabStepCacheBad` instead violates `Prop` (2/2/2,
exit 13, independently validated trace `x=0 -> x=1`). `EnabStepSearch`
searches `x' \in 0..2` for `NX = 3`; its witness occurs only on the third
candidate, so it exercises the step-clear boundary independently, with an
impossible `NX = 4` query as a negative control. Pinned TLC gives success
(3/4/3) for both holding fixtures. `core_enabled_step` checks both profiles,
1/4/8 workers, and the keyed and step memo tables independently on/off.
Removing cache isolation fails the first test; retaining `step` while
compiling the query fails the second. Restoring both passes both tests.

**Model.** BufferedRandomAccessFile matches its golden on the core: 3316 distinct, 128581 generated, depth 10. M3, 1 worker, indicative: core 10.5 s user, reference engine 79.0 s (`crates/dive-api/tests/core_golden.rs`).

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

### 6.1 One-worker speed: GameOfLife and the slowdown since M0

GameOfLife's `Sum(sc, points)` folds `sc[x]` over a set of pairs, removing one element at a time (`S \ {x}`). `grid` and `sc` are functions on pair domains. The core keeps `sc` materialized, once per activation (§7). Its cost was in three places. They are fixed at the value level and are exact:
- **Pair-domain lookups.** `f[<<a, b>>]` on a function of more than 8 keys did a binary search, with a deep comparison per probe. When the domain is a dense rectangle `l1..h1 \X l2..h2` (the first and last keys give the bounds, and the size checks it), the position is computed and then checked against the key found there (`Machine::grid_index`). Any other domain falls back to the search.
- **Small differences.** `S \ T` with `T` smaller than `S` looks up each element of `T` in `S`, instead of each element of `S` in `T` (`Machine::minus_small`). The kept elements still go through the failed-membership check (`compare::miss`), so its errors are those of TLC's `SetDiff`.
- **Comparing tuples.** Two integer-domain functions with the same domain compare by their values only.

Fixture `GridFcn` (TLC: `Inv` violated, 1665/5174/6) covers a rectangle, a near-rectangle whose computed position misses, and a triangle.

GameOfLife on M3, 1 worker, instructions retired:

| Build | Instructions |
|---|---|
| M0, `866c0978` | 101.2 G |
| main, `0abc2844` | 113.4 G |
| this change | 79.5 G |
| pinned TLC, JVM start included | 153.8 G |

At load 12, the core took 5.1 s wall and 4.8 s user, against TLC's 28.4 s and 15.3 s.

**One-worker slowdown since M0** (lead's x86 bench: MCLamportMutex 10.47 to 12.53 s, GameOfLife 10.44 to 11.73 s, SimpleRegular 2.58 to 2.98 s). Bisected on M3 by instructions retired, core engine, 1 worker:

| Commit | SimpleRegular | GameOfLife | MCLamportMutex |
|---|---|---|---|
| M0 `866c0978` | 24.4 G | 101.2 G | 89.9 G |
| #508 `07e0fd8d` | 24.7 | 101.4 | 90.3 |
| #509 `d344f0a3` (comparison errors, stream 2) | 25.1 | 110.5 | 92.1 |
| #510 `57e9e679` (store columns) | 25.9 | 112.7 | 94.7 |
| #524 `431d6f95` | 26.1 | 113.1 | 96.2 |
| #526 `ddcbb6c1` (string tokens) | 26.1 | 113.6 | 97.0 |
| #530 `0bff25aa` (memo off, stream 2) | 25.4 | 113.7 | 110.5 |
| #545 `d81170b2` (#535 to #545) | 26.9 | 114.5 | 114.1 |
| `0abc2844` | 26.8 | 113.9 | 114.4 |

- **Mine, fixed.**
  - #530 turned the memo tables off on wall-clock evidence taken under load. That cost MCLamportMutex 13.4 G. The tables are back on (`06-memoization.md` §6).
  - #509's failed-membership checks cost GameOfLife 9 G. The paths above more than recover it.
- **Others, reported.**
  - #510's size-classed store: about 2 to 3 G on each model.
  - The range #535 to #545: 2.6 G on MCLamportMutex and 0.8 G on SimpleRegular.
  - #526's string tokens: 0.8 G on MCLamportMutex.

## 7. Next step

The #623 constructor path now consumes actual scalar/model-value/nested-tuple
literal, MAP and covered union histories, including newly assigned successor
queries and sparse normalized state views. Constructor/consumer history is
ineligible for speculative folding, keyed memoization, extra definition reuse
and parent skips; actual LET thunk sharing remains. The old closed-predicate and
canonical-producer source certificates are not widened. The original 18 primary
controls remain, alongside twelve TLC models/twelve SANY inputs in
`conformance/corpus/construction-set-history`. All goldens come from the official
harness with recorded reasons. Covered old positive controls now require exact
primary agreement; failing C2 cases and unproved E7 classifications still refuse.
Actual persistent view ownership and rollback are described in
`03-value-model.md` §8 and ADR 0131. Whole-model Sailfish performance remains
unqualified; this is not the generic S5 index.

The next checked-comparison delivery follows three dependencies:

1. Preserve covered raw literal/map/union construction order and duplicates,
   actual normalization demand/history, receiver class and alias lifetime
   through imports, arguments, memo/fold and state handoff. Persistence,
   `Kind::Set` and `is_norm` do not establish TLC history. Sailfish can query
   newly assigned `es'` before commitment; a predecessor's normalized status
   does not certify that successor set.
2. Consume exact closed scalar/tuple normalization at the actual demand:
   retain TLC's insertion order and first equal representative. Raw membership
   scans member-to-query checked equality; a proved normalized view uses
   query-to-pivot checked comparison. Stop at the first hit/error. Keep the
   twelve #619 controls consumed and all uncovered paths guarded/refused;
   metadata or an isolated helper alone is not a delivered live port.
3. Widen record/function/nested/symbolic classes one bounded family at a time.
   E7 diagnostic coverage separately needs receiver construction/source
   ownership and replay-lifetime provenance. #620 is already unverified in
   the reference and `Refused` by the native reader, not a false agreement;
   no classifier mapping or exemption follows from it.

The bounded witness memo is not the generic S5 index, which remains unbuilt.
Hidden/nonidentity INSTANCE queries and B2/B4 lifetimes remain separate work.
The earlier M0/census ordering below is historical context; current boundaries
and completed work stay recorded in §§4–5 and §8.

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
  eager arguments. Function constructors are materialized. (M1: see §4.1.)
- **Folding.** Closed subexpressions are folded by evaluating them at compile
  time, within the R9 bound (§3, pass 1). A folding failure leaves the node
  alone. Filters and `CHOOSE` enumerate a symbolic domain through its cursor
  instead of materializing it first.
- **Errors.** Evaluation errors are reported as `unsupported`. (M1: the
  core classifies them, §5.2; the engine does not yet report them.)

Next (M1, stream 2), from the census:
1. Error classes (§5.2): typed classes are built, and the engine reports
   error endings by reproduction (stream 3). Next, cross-check the core's
   class against the reproduced one. Then extend classification to the
   unclassified sites, checking each against a pinned-TLC probe first.
2. `INSTANCE` substitution: built (pass 2). Next comes EWD998PCal's
   speed. It needed Bags (below), and its properties are still the
   engine's (M3).
3. Operator breadth in census order: `TLC!Print`/`PrintT`/`Assert`/
   `ToString`, which need effects (B6), and the standard modules.
   - **Done: Bags.** `EmptyBag`, `SetToBag`, `BagToSet`, `BagIn` and
     `CopiesIn` are in (`Machine::bag`). Fixture `Bag.tla` gives 27 distinct,
     111 generated, depth 5 with the pinned TLC, the reference engine and the
     core.
   - **Done: `ENABLED`** (§5.5), including constant/operator instance
     parameters with proved identity variable mappings. Next: hidden and
     nonidentity instance variables, with TLC's branch cutoff (B3) in both
     profiles and the B2/B4 cache rules under `--compat=tlc` (register D-027
     when the approved sound default becomes observable).
   - **EWD998PCal**, safety with its constraint and without its properties
     and alias, now runs in the core with the reference engine's counts:
     321,370 distinct, 2,808,396 generated, depth 47. M3, indicative, load
     11: 2.9 s user and 30 MB RSS, against 11.0 s and 127 MB for the
     reference engine.
   - **Bags, the rest (stream 1, `stdlib/bags.rs`):** `\oplus`,
     `\ominus`, `BagCardinality`, `BagUnion`, `\sqsubseteq` and `IsABag`.
     Fixtures are in `tests/fixtures/core-lib` (§8, library fixtures).
     Still to do: `BagOfAll` and `FoldBag` (BagsExt), which take operator
     arguments.
   - **TLC, values (stream 1, `stdlib/tlc.rs`):** `:>`, `@@`, `Assert`,
     `Permutations` and `ToString`. `ToString` prints through
     `crates/dive-core/src/print.rs`: a core value converts to the reference
     engine's value (symbolic sets stay symbolic) and `dive_value::print`
     prints it, with the model's names (`Store::set_printer`). The printed
     argument keeps its form (an interval prints as `1..3`), but the core
     enumerates intervals and lazy sets inside other values, in states, and
     as `DOMAIN` of a sequence, where TLC keeps them. So when the plan may
     build one (`a..b`, `DOMAIN`, `SUBSET`, `[S -> T]`, `[a : S]`,
     `S \X T`, or a constant holding one: `Store::note_lazy_sets`),
     printing refuses an enumerated set of consecutive integers, or of at
     least 128 elements (`TsStored`, `TsDomain`, `PrAmbig`). `Print` and `PrintT` print the same text (pretty-printed,
     as the reference engine prints it), under the effect bit (§3, pass 6).
     `TLCSet` and `TLCGet` are done (`03-value-model.md` §6.2). Still to
     do: printing
     with several workers (each state's output buffered and released in
     TLC's order, as the reference engine does), output with a violation
     (TLC's trace-rebuilding prints). `RandomElement` is done
     (`03-value-model.md` §6.1): the reference engine's streams, one
     worker, and a `LET`-bound effect used in more than one conjunct
     refused.
     `SortSeq` is done (operator arguments, below).
   - **CommunityModules, values (stream 1, `stdlib/community.rs`):** see
     §8. `AllSubSeqs`, `LongestCommonPrefix`, and `ReplaceFirstSubSeq` and
     `ReplaceAllSubSeqs` on strings are in, with `Len` of a string (UTF-16
     units, as Java counts). `AllSubSeqs` builds TLC's unnormalized set
     under TLC's comparisons, where membership in it is refused (C1); the
     default profile normalizes it, as D-009 answers membership. A string
     shorter than the first in TLC's string order (`LongestCommonPrefix`,
     TLC's Java exception) is refused. Fixtures `CmSubSeqs` (285 distinct,
     891 generated, as TLC), `CmSubSeqsViol` (143 distinct, 168
     generated, 131 left, as TLC), `ErCmLcpEmpty`, `ErCmLcpNonSet` and
     `ErCmAllSubSeqsNonSeq`. Sequence replacement now supports pure,
     materialized sequences with one scalar element class across all three
     arguments (integers, booleans or strings), with bounded linear matching.
     ReplaceAll refuses overlapping occurrences after its early CASE arms;
     see `03-value-model.md` §12. Still to do: mixed, compound, lazy and
     effectful sequence replacement arguments, overlapping All occurrences,
     and `FiniteSetsExt`'s
     `kSubset`, which the reference engine does not implement either.
   - **Operator arguments (stream 1, `stdlib/higher.rs`, `Node::LibHo`):**
     `SelectSeq`, `SortSeq`, `BagOfAll`, `FoldBag`, `Quantify`,
     `FoldFunction`, `FoldSeq`, `FoldLeft`, `FoldRight` and their `Domain`
     forms, `SelectInSeq`, `SelectLastInSeq`, `RemoveFirstMatch`,
     `FoldSet` and `FoldFunctionOnSet`. The last two walk TLC's
     `elements()` of the set, which the pinned TLC gives in the normalized
     order, without repeats, for every set probed: `S \cup T` with
     overlap, `UNION`, `S \ T`, set maps, `SUBSET`, `[S -> T]`, `S \X T`
     and strings, in actions, invariants and `ASSUME`s, in both profiles.
     The core folds in its normalized order. Fixtures `FoldOps` (55
     distinct, 256 generated, as TLC), `ErFoldOnSetNonFcn`,
     `ErFoldOnSetNonSet` and `ErFoldOnSetDomain` (2154, the application
     error inside the override). A `FoldSet` argument that is not an
     enumerable set (TLC's `ClassCastException`) is refused. Still to do:
     `MapThenFoldSet` and the `Folds` module (TLA+ definitions with
     `CHOOSE` and recursion, no override), and the `SubSeq` forms of the
     selections (their argument-not-in-domain error class is not mapped
     yet).
4. The memoization decision: made, then revised. The memo tables are on
   by default (`06-memoization.md` §6).
5. Native action splitting and constant evaluation, to drop the reference
   binder from the handoff.
6. Model values: built (§5.3). Of the 23 small-tier models refused for
   model values or value overrides, 16 now agree with the reference engine
   once their properties are removed (MCMajority among them).
   - Two are rejected in both engines.
   - SlushSmall has no JSON result in either.
   - The rest are refused for other reasons, which are the next work, in
     census order:
     - operator arguments (CigaretteSmokers), constant operators
       (MCInternalMemory) and nested operator substitutions (TLCSailfish1):
       built (§5.4);
     - `TLC!PrintT` (MCEcho, effects B6): the stdlib (stream 1).
7. The split-argument refusal ("a split argument that is not constant").
   The small tier had 6 models refused this way: MCDieHarder,
   MCDisruptor_MPMC, ElevatorSafetySmall, SlidingPuzzles, MCbtree and
   kvstore.
   - It was not a resolution bug. The 2280 "undefined or not an operator"
     in the message came from the reference binder evaluating a split
     action's `LET` value (`isLeaf`, `next_sequence`) outside its scope.
     With `LET` values bound lazily (§2.1), MCbtree, MCDisruptor_MPMC,
     MCDieHarder and kvstore agree with their goldens.
   - The nested substitution TLCSailfish1 needed is built (§5.4).
   - `crates/dive-api/tests/core_golden.rs` checks MCbtree,
     MCDisruptor_MPMC and TLCSailfish1 against the pinned goldens at 1 and
     4 workers.
   - Next: constant evaluation, to drop the reference binder from the
     handoff.

GameOfLife's `sc` has a product domain, so TLC keeps it lazy. The core still
caches its materialization once per activation, under `Node::Spec`, so its
speed is unchanged. Laziness is not GameOfLife's speed answer. Profiled, its
cost was pair-domain lookups and set differences, now fixed (§6.1).

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

**Library fixtures** (`crates/dive-api/tests/fixtures/core-lib`, test
`core_rules::library_fixtures_match_the_reference_and_tlc`). There is one
model per operator family of `crates/dive-core/src/stdlib/`, and one per
error case. Each has a pinned-TLC transcript (`dive-conformance dev
capture`). The core must agree with the reference engine and, on success,
with TLC's counts.

| Family | Operators | Fixtures | Result |
|---|---|---|---|
| Bags | `\oplus`, `\ominus`, `BagCardinality`, `BagUnion`, `\sqsubseteq`, `IsABag` | `BagOps` (32 distinct, 95 generated, depth 9; TLC agrees); errors `ErBagCardNonBag` (2176), `ErBagCupNonBag` (2169), `ErBagDiffNonFcn` (2169), `ErBagSqNonFcn` (2176) | all agree; `ErBagUnionNonBag` (TLC 2154, comparing `2` with `<<1>>`) is refused: a set of an integer and a function (C1, C2) |
| TLC | `:>`, `@@`, `Assert`, `Permutations`, `ToString`, `Print`, `PrintT` | `TlcOps` (48 distinct, 145 generated, depth 9), `Perm` (6, 42, depth 1), `PermViol` (violation after 36 generated, trace 3), `TsOps` (4 distinct, 13 generated; 16 printed forms, among them `Seq({1})`, `[a |-> 3]` for `"a" :> 3` and `<<3>>` for `1 :> 3`), `TsLazy` (1 distinct, 2 generated; `1..3`, `SUBSET (1..7)` and small lazy sets expanded), `PrOps` (4 distinct, 11 generated, 58 printed lines in TLC's order: start-up, initial states, constraints, invariants, a `LET` forced once, a definition used twice), `PrRange` (2 distinct, 4 generated, 6 lines), all equal to TLC; errors `ErAssertInv` (2132 in an invariant), `ErAssertNext` (assertion failure in the next-state relation, exit 14), `ErCombineNonFcn`, `ErCombineSecond` (2169), `ErPermNonSet` (2176) | all agree. `Permutations` is refused over strings or model values in the default profile and over compound elements: TLC enumerates the result in its own order (D-010). `TsStored` (`ToString` of a stored `1..2`), `TsDomain` (`DOMAIN` of a sequence), `PrAmbig` (`{x}` printed where intervals occur), `PrViol` (output with a violation) and `PrLazy` (`PrintT` in a function constructor) are refused |
| Operator arguments | `SelectSeq`, `SortSeq`, `BagOfAll`, `FoldBag`, `Quantify`, `FoldFunction`, `FoldSeq`, `FoldLeft`, `FoldRight`, `FoldLeftDomain`, `FoldRightDomain`, `SelectInSeq`, `SelectLastInSeq`, `RemoveFirstMatch` (as LAMBDAs, definitions, LET operators and operator parameters) | `HoOps` (121 distinct, 364 generated; TLC agrees); errors `ErHoSelectSeqNonSeq`, `ErHoFoldLeftNonSeq` (2169), `ErHoInBody` (2179 inside the operator) | all agree. Passed an operator whose body is a numeral or a string literal (`LAMBDA y : 0`), TLC passes the value, so the overrides that take an operator fail (`BagOfAll` 2266, `SelectSeq` and `SortSeq` 2169, the `OpValue` parameters 2154). The reference engine does the same since #557 (fixtures `crates/dive-api/tests/fixtures/op-literal`); the core still refuses the call (`ErHoConstBagOfAll`, `ErHoConstFoldLeft`) |
| CommunityModules, values | `SequencesExt`: `Contains`, `RemoveFirst`, `Suffixes`, `SetToSeq`, `SetToSeqs`, `IsPrefix`; `Functions`: `IsInjective`, `AntiFunction`; `Bitwise`: `And`, `Or`, `Xor`, `Not`, `shiftR` | `CmOps` (16 distinct, 51 generated, depth 4), `CmSeqsViol` (violation after 543 generated, whose counts depend on `SetToSeqs`' order), both equal to TLC; errors `ErCmContainsNonSeq`, `ErCmInjectiveNonFcn` (2283), `ErCmRemoveFirstNonSeq` (2169) | all agree. Refused as the reference engine refuses them: Java exceptions (`SetToSeq` and `IsPrefix` of a non-sequence, Bitwise of a non-integer or beyond 32 bits) and `IsInjective` over values TLC cannot compare |

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

- **Stream 1, `core/stdlib-bags`: the library hook.**
  - `plan.rs`: `Node::Lib { op, args, repeatable_args }`, a call of an
    operator in `crate::stdlib` on normalized arguments. The compiler records
    whether the arguments are effect-free without calls or free slots;
    non-string sequence replacement refuses unproved repeatability because
    TLC's fallback demands the arguments again.
  - `compile.rs`: `std_apply` emits it for any `StdOp` that
    `stdlib::implements` accepts and the compiler does not build itself;
    `info_of` walks its arguments.
  - `eval.rs`: evaluates the arguments with `eval_norm` and calls
    `stdlib::call`.
  - Contract: a library operator is a function of normalized values.
    `stdlib::effectful(op)` is true for an operator with an effect (`Print`,
    `TLCSet`, randomness); none is implemented yet. When one is, the
    compiler will treat its node as not foldable and not memoizable.
    Operators with operator arguments use `Node::LibHo(StdOp,
    Box<[LibArg]>)`: `compile.rs` `lib_ho` compiles each operator argument
    with `op_fn` at `stdlib::op_arity`, and `eval.rs` passes
    `stdlib::OpFn`s, which bind the slots, evaluate the body (normalized)
    and restore the slots. For an override that takes an `OpValue`
    (`stdlib::takes_op_value`), an operator whose body compiles to a
    constant is refused (§8).

- **Stream 3, `core/engine-getnext`.**
  - `plan.rs`: new generator nodes `AssignInRaw`, `StepAngle` and
    `FcnAction`; `Unchanged` and `StepBox` hold `Unch` items (`Var` with its
    site, or `Verify(P)`); the `Raw` tree; `Plan::action_constraints`.
  - `compile.rs`: `Handoff::action_constraints`; the generator arms of `g`
    for `\in`, `\subseteq`, `UNCHANGED`, `[A]_v`, `<<A>>_v` and `f[a]`; the
    value forms of `[A]_v` and `<<A>>_v` in `p`. The new helpers live in
    `compile_gen.rs`, a child module of `compile`.
  - `eval.rs`: `Machine::warnings`.

