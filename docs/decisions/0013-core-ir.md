# 0013: Core IR (`dive-ir`)

- **Status:** proposed
- **Date:** 2026-10-08

## Context

`dive-ir` is the resolved, lowered form of a specification. Every backend consumes it:
- the evaluator and successor generator (`dive-eval`, ADR 0014);
- the checker, simulator and liveness checker, through the evaluator;
- the independent trace validator (`dive-trace`, ADR 0016);
- native compilation (`dive-compile`);
- a future exporter to Verus state machines compatible with verus-tla.

Its input is `dive-sema`'s semantic model (in progress on its branch). That model is SANY's semantic graph as an arena:
- `OpAppl` nodes name built-ins by SANY's names (`$SetEnumerate`, `$BoundedExists`, ...);
- instantiated definitions are wrapped in `SubstIn` nodes;
- `LET`, labels, `@` and proofs are kept;
- level-checking results are stored on every node.

Lowering is therefore a port of how TLC walks SANY's graph, and this ADR fixes what that port must preserve.

TLC paths below are relative to `tlatools/org.lamport.tlatools/src/` at tlaplus `94d0c50`. Probes are in `crates/dive-check/tests/fixtures/tlc-probes/`.

### TLC behaviour the IR must be able to express

1. **Call-by-need.**
   - Arguments of user-defined operators are passed as cached `LazyValue`s (`SymbolNodeValueLookupProvider.java:124-146`).
   - Each zero-arity `LET` definition becomes a fresh `LazyValue` every time the `LET` is evaluated (`Tool.java:1849-1861`).
   - `INSTANCE` substitutions are bound lazily in the context (`Tool.java:1864-1873`).
   - A cached value is reused only under TLC's compatibility test: same tool, states that agree on every assigned variable, equivalent `EvalControl` (`LazyValue.getCachedValue`; `TLCState.java:246-276`; `EvalControl.java:92-110`).
   - Laziness is observable. An unused argument that would fail raises no error, and `Print`, `TLCSet` and `RandomElement` run once per lazy instance, not once per use.
2. **Constant definitions are evaluated once.**
   - Every zero-arity definition whose body is constant-level is evaluated at start-up and cached. A failure is swallowed, and the definition stays lazy (`SpecProcessor.java:341-381`).
   - `C <- Op` overrides evaluate `Op`'s body once (`:274-333`).
   - So `R == RandomElement(S)` at constant level is drawn once per run.
3. **Splitting the next-state relation into actions** (`Tool.getActions`, `Tool.java:269-419`).
   - `\/` splits.
   - `\E` over a domain that evaluates at constant level splits into one action per binding, in enumeration order. If the domain is empty or fails to evaluate, the whole `\E` is one action.
   - A user-defined operator whose arguments are all constant-level is inlined, and the actions inside it take its name.
   - `LET` and labels are transparent. A `SubstIn` with substitutions is not split.
   - Anything else is one action.
   - Actions are numbered after the initial predicates (`Tool.java:225-241`).
   - Each action's printed label is `<Name(params) line L, col C to line L2, col C2 of module M>` (`Action.java`; `tla2sany/st/Location.java:344-360`).
   - Probe `EvalErrorMidLevel` shows a split action labelled with its operator's name (`A`).
4. **Initial predicates.**
   - With `INIT`/`NEXT`, `Init` is one predicate and is not split (`SpecProcessor.java:954-998`).
   - With `SPECIFICATION`, the formula is walked through conjunctions, `SubstIn`, `LET` and operator applications. Level-≤1 conjuncts become initial predicates. `[][A]_v` supplies the next-state action `A`, unnamed (`SpecProcessor.java:1044-1390`).
   - Probes `StutterSpec` and `StutterInNext` confirm the consequence:
     - `SPECIFICATION Init /\ [][Next]_x` explores `Next`. It generates no stuttering steps and can deadlock.
     - `NEXT [Next]_x` explores `[Next]_x`. It generates and counts a stuttering successor for every state.
5. **Java overrides replace definitions.** TLC installs a Java implementation for many standard-module operators and ignores their TLA+ bodies (`SpecProcessor.processModuleOverrides`, `:2114-2324`). Some bodies are placeholders: CommunityModules `Json!ToJson` is `CHOOSE` over `STRING` (survey probe P3).

### Constraints from the other consumers

- **Verus export** (`docs/survey/adjacent.md`).
  - verus-tla models an action as a guard plus a deterministic transition, with nondeterminism lifted into an `Input` parameter.
  - So the IR must keep three things recoverable:
    - `Next` as a disjunction of named actions;
    - each action's existential parameters;
    - each action's per-variable primed assignments (`x' = e`, `x' \in S`, `UNCHANGED`), separate from its guards.
  - Stuttering must be explicit. `SF_`, `\EE` and `\AA` have no counterpart.
- **Compilation** (`dive-compile`). Generated Rust cannot do name lookup at run time, and closures are only cheap when the binding structure is static. The IR therefore needs:
  - resolved references and fixed slot layouts;
  - explicit laziness points;
  - explicit recursion;
  - effect annotations that say which subexpressions may be reordered, shared or evaluated early.
- **Layering** (`xtask/src/layering.rs`). `dive-ir` may depend only on `dive-syntax` and `dive-sema`. It cannot hold `dive-value` values or read `dive-config`.

## Decision

### 1. One arena per specification, immutable after lowering

`dive_ir::Ir` holds every lowered module of one specification: definitions, expressions, declarations, origins and analyses. It is immutable after `lower(&Analysis) -> Result<Ir, Vec<Diagnostic>>`, and it is `Send + Sync`. It is shared behind an `Arc`.

| Id | Indexes |
|---|---|
| `DefId` | operator, function and theorem definitions, including instantiated (`I!Op`), `LET`-local and `LAMBDA` definitions |
| `ExprId` | expression nodes |
| `VarId` | state variables of the root module, in declaration order |
| `ConstId` | `CONSTANT` declarations of the root module and of instantiated modules |
| `Slot` | a position in a definition's frame (parameters, bound variables, `LET` locals, substitution bindings) |
| `StdOp` | an operator that TLC overrides in Java (§5) |

Ids are dense `u32`s assigned by a deterministic traversal: modules in dependency order, definitions in source order. Lowering the same input always gives the same ids. Ids are not stable across edits; §7 covers caching.

### 2. Expressions

Expression kinds are explicit Rust enum variants, not SANY's `$`-names. A spec SANY accepts lowers to no error nodes.

| Group | Variants |
|---|---|
| Literals | `Bool`, `Num` (the numeral's text; the integer mode decides at binding time whether it is too big, `TLC_INTEGER_TOO_BIG` under `--int=tlc`), `Str` (unescaped as SANY does) |
| References | `Var { var, primed }`, `Const(ConstId)`, `Local(Slot)`, `SubstRef { slot, primed }`, `Def(DefId)` (zero-arity use) |
| Application | `Apply { callee, args }`, where `callee` is `Def(DefId)`, `Param(Slot)` (an operator parameter), `Std(StdOp)` or `Lambda(DefId)`, and each argument is an expression or an operator reference |
| Logic | `Junction { kind: And/Or, bulleted: bool, items }` (n-ary, item order kept), `Not`, `Implies`, `Equiv`, `If`, `Case { arms, other }` |
| Binders | `Quant { kind: Exists/Forall/Choose, bounds, body }`, where each bound is a single slot or a tuple pattern with an optional domain (unbounded forms are kept so that evaluation can raise TLC's error); `SetFilter`, `SetMap`, `FcnCons`, `RecFcn` (recursive function definitions `f[x \in S] == e`) |
| Data | `SetEnum`, `Tuple`, `Record`, `RecordSet`, `FcnSet`, `Cartesian`, `FcnApply`, `Except { base, updates }` with `ExceptAt` for `@`, plus the set operators as `Std` or language built-ins |
| Scoping | `Let { defs, body }`; `Subst { bindings, body }` for instance substitutions; `Label { name, body }` (transparent, kept for diagnostics) |
| Actions | `Prime(e)` for primed non-variables, `Unchanged(e)`, `StepBox { action, sub }` (`[A]_v`), `StepAngle { action, sub }` (`<A>_v`), `Enabled(e)`, `Cdot(a, b)` |
| Temporal | `Always`, `Eventually`, `LeadsTo`, `Wf`, `Sf`, `TemporalExists`, `TemporalForall` |

**Every expression node carries:**
- `origin`: file, text range and module, plus the instance chain for instantiated definitions;
- `level`: SANY's level (constant, state, action, temporal), taken from `dive-sema`;
- `effects` (§6).

**Frames.**
- Each definition owns a frame layout: parameters, then bound variables, `LET` locals and substitution bindings, each with a fixed slot.
- `LET` definitions with parameters, and `LAMBDA`s, are separate `DefId`s marked `Local { parent }`. They capture their parent's frame.
- Nothing is looked up by name after lowering.

### 3. Laziness, `INSTANCE` and recursion

- **Laziness points are explicit.** These are the places TLC creates a `LazyValue`:
  - user-operator arguments;
  - zero-arity `LET` definitions;
  - `Subst` bindings.

  The evaluator implements TLC's caching rule at those points (ADR 0014).
  - Lowering marks an argument or binding `Strict` only when the effects analysis shows it is pure and cannot fail: a literal, a variable, a bound name, or a tuple or record of these.
  - Evaluating such a node eagerly is indistinguishable from TLC.
- **`INSTANCE` is not expanded by textual substitution.**
  - An instantiated definition `I!Op` is a `DefId` whose body is the original body under a `Subst` node binding the instance's substitutions. Priming a substituted variable (`x <- e`, then `x'`) lowers to `SubstRef { primed: true }`, meaning `e` evaluated in the primed context.
  - This keeps TLC's lazy, cached substitution semantics. It also avoids the blow-up of copying bodies per instance, and keeps the `ENABLED`-under-substitution cases fixed upstream in `8a23cb1dc` expressible.
  - The pass `inline_pure_substitutions` replaces a binding by its expression when the expression is pure and cannot fail. That is semantics-preserving, and it lets the evaluator and compiler see through common refinement mappings.
- **Recursion.**
  - `RECURSIVE` operators are ordinary definitions marked `recursive`. The call graph may be cyclic only through them.
  - Recursive functions are `RecFcn` nodes.
  - Evaluation depth is bounded by the evaluator and reported as a resource limit, never a crash (ADR 0014).

### 4. Actions

**The action tree.** `dive_ir::actions::split(expr) -> ActionTree` mirrors `Tool.getActions` exactly. Its nodes:
- `Disj(children)`;
- `ExistsSplit { bounds, body }`: a `\E` whose domains are constant-level;
- `Inline { def, args, body }`: a user operator with constant-level arguments;
- `Leaf(ExprId)`.

`LET`, labels and substitution-free `Subst`s are transparent. A `Subst` with substitutions is a leaf.

**From tree to actions.** Expanding `ExistsSplit` needs values, so it happens when the model is built (§8). The result is the flat, ordered action list TLC uses. Each action has:
- a leaf expression;
- constant bindings for its split variables and inlined parameters;
- a name;
- an origin;
- a sequential id.

If a split domain fails to evaluate or is empty, the whole `\E` becomes one action, as in TLC.

**Leaf node kinds.** The successor generator interprets leaves with TLC's `getNextStates` recursion (ADR 0014). The IR guarantees that every node kind that recursion distinguishes has its own variant and keeps its source order:
- junction items;
- `x' = e`, `x' \in S`, `x' \subseteq S`;
- `UNCHANGED` of a variable, a tuple or a zero-arity definition;
- `[A]_v`, `<A>_v`, `IF`, `CASE`, `\E`, `\A`, `=>`;
- application of a function-valued lambda.

**The `ActionShape` view.** `dive_ir::actions::shape(action)` returns an `ActionShape` for export and diagnostics:
- `name`;
- `params` (split variables and their domains);
- `guards` (state-level conjuncts);
- `updates`: a map from `VarId` to `Assign(e)`, `In(set)`, `Subseteq(set)` or `Unchanged`;
- `residual`: action-level conjuncts that are not single-variable updates.

An action that is not a conjunction of guards and updates, after inlining, is `Opaque(reason)`. Checking never uses this view; it is not on the path to any verdict. The Verus exporter will:
- expand `[A]_v` to `A \/ UNCHANGED v`;
- refuse opaque actions, `SF_`, `\EE` and `\AA` with `unsupported`.

### 5. Standard modules and Java overrides

**`StdOp`.** An operator defined in a standard module that TLC overrides in Java lowers to `Callee::Std(StdOp)`. `StdOp` is an enum in `dive-ir`, one variant per overridden operator in:
- Naturals, Integers, Sequences, FiniteSets, Bags, Strings;
- TLC, TLCExt, Randomization, Json;
- `_TLCTrace`, `_JsonTrace`, `_Possible`;
- the CommunityModules operators dive implements.

The TLA+ body is kept for documentation, but nothing evaluates it unless `dive-stdlib` declares the body equivalent to TLC's override and a conformance test proves it.

**Matching and refusal.**
- Matching is by module and definition, as TLC's own matching is. A user module's definition that has the same name is not a `StdOp`.
- An overridden operator that dive does not implement is refused as `unsupported` when the model is built, if it is reachable from any role (§8). It is never evaluated through its placeholder body.

### 6. Effects analysis

Lowering computes, bottom-up over expressions and to a fixed point over definitions:

| Effect | Set by |
|---|---|
| `impure` | `Print`, `PrintT`, `TLCSet`, `RandomElement`, `Randomization`, `JavaTime`, `TLCGet` of a register or a live counter, CommunityModules I/O |
| `live_tlcget` | `TLCGet` of `"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"duration"`, `"stats"`, `"all"`, `"all:named"` or an integer or `"s:"` register |
| `may_fail` | conservatively, anything that can raise an evaluation error |
| `uses_enabled`, `temporal` | `ENABLED` and temporal operators |

The analyses decide:
- strict versus lazy points (§3);
- substitution inlining (§3);
- whether the checker can run in parallel or must fall back to sequential mode (ADR 0015);
- what `dive-compile` may hoist or share.

### 7. Origins, stable identities and serialization

- **Origins.**
  - Every node's origin maps to file, line and column for diagnostics, using `dive-syntax`'s SANY column rules.
  - It also gives the TLC-format location string used in counterexample labels and in the `-dumpTrace` JSON `location` objects.
- **Structural hashes.**
  - Each definition has a hash of its lowered body that ignores origins. `dive-compile` uses it for its build cache, and IR caching uses it for invalidation.
- **Serialization.**
  - The IR derives `serde` serialization behind a version constant `IR_SCHEMA`.
  - The on-disk cache is keyed by SHA-256 over: dive's version, `IR_SCHEMA`, the bytes of every loaded module, the search path and the lowering options.
  - A cache hit must give an `Ir` equal to a fresh lowering; a test asserts this for the corpus.
  - Caching is an optimisation. The ADR 0006 start-up target (≤ 10 ms for a trivial spec) is met without it.

### 8. Binding the configuration: a new crate, `dive-model`

The IR is value-free and configuration-free. Constants, overrides and roles are bound in a separate stage, `dive-model`, which ports TLC's `SpecProcessor`.

**Input:**
- `Arc<Ir>`;
- the parsed `.cfg` (`dive-config`);
- `dive-value`'s `Ctx` (integer mode, value order);
- the `dive-stdlib` table.

**Output:** a `Model` holding:
- **Constant bindings:**
  - each `ConstId` is bound to a `Value` or to a `DefId` (`C <- D`). Model values are interned in the cfg's creation order (`ModelConfig::model_values`);
  - `<-` overrides of operators, as a redirect table. The IR is not rewritten, so it stays cacheable independently of the cfg;
  - the precomputed constant definitions, under TLC's rules (Context item 2), with failures left lazy.
- **Roles:**
  - initial predicates and next-state actions (the expanded action list of §4);
  - invariants, implied initial predicates and implied actions;
  - temporal properties (refused in M5);
  - `CONSTRAINT` and `ACTION_CONSTRAINT`;
  - `VIEW`, `SYMMETRY`, `ALIAS` and `POSTCONDITION`;
  - the deadlock flag.
- **Errors:** every configuration and specification mismatch, with TLC's `EC` code and message (`TLC_CONFIG_*`, `TLC_CANT_HANDLE_*`, `TLC_INVARIANT_VIOLATED_LEVEL`, ...).

**Why a separate crate.** The binding stage needs `dive-config`, `dive-value`, `dive-eval` (to precompute constants and expand splits) and `dive-stdlib`. `dive-ir` may use none of them, and `dive-eval` may not use `dive-config`. The checker, the simulator, the liveness checker and the trace validator all need the same binding. Duplicating a `SpecProcessor` port in each would be a soundness hazard.

**Proposed layering change** (an owner decision; `layering.rs` is not edited here):
- add `("dive-model", &["dive-ir", "dive-value", "dive-config", "dive-eval", "dive-stdlib"])`;
- add `dive-model` to the allowed dependencies of:
  - `dive-check`, `dive-liveness` and `dive-sim`;
  - `dive-trace`. This is permitted by the trace-independence rule, which only excludes the checker, liveness checker, simulator and store; see ADR 0016;
  - `dive-compile`;
  - `dive-api`.

### 9. Deliberately out of scope for M5

- Type inference. A side table for optional type hints is reserved for the Verus exporter.
- Common-subexpression elimination.
- Constant folding beyond TLC's own precomputation.
- Inlining of user operators beyond §3 and §4.

Each of these can change evaluation order, error behaviour or side effects, so each needs its own ADR and a semantics-preservation argument.

## Consequences

- The evaluator, the trace validator and the compiler see one resolved form with explicit laziness, so call-by-need and TLC's error behaviour are implemented once, at known points.
- Because `INSTANCE` is kept as substitution frames, refinement-heavy specs (for example Examples `allocator`, `Paxos`) do not grow with the number of instances. The evaluator pays for substitution lookups instead, which `inline_pure_substitutions` reduces.
- The action tree reproduces TLC's action list. Generated-state counts and counterexample labels can therefore match: they depend on how `Next` is split.
- The Verus exporter can be built later from `ActionShape` without changing the IR. It refuses what it cannot express.
- `dive-model` is a new crate and a layering change. Until the owner approves it, the binding stage cannot be shared, so the M5 plan sequences it early (`docs/design/m5-plan.md`, WP-B1).
- The IR depends on `dive-sema`'s model shapes. A change there means a lowering change, and the corpus round-trip test (lower everything SANY accepts) catches it.
- Revisit:
  - when the Verus exporter starts (type hints);
  - when `dive-compile` starts (whether frames need more structure);
  - if profiling shows substitution frames dominate evaluation.
