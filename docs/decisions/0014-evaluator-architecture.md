# 0014: Evaluator architecture (`dive-eval`, `dive-stdlib`)

- **Status:** proposed
- **Date:** 2026-10-08

## Context

The evaluator turns the IR (ADR 0013) and a bound `Model` into values, truth values and successor states. The conformance contract puts unusual demands on it:

- **Generated-state counts.** These are compared with TLC's. They count every successor TLC's enumeration produces, duplicates included (probe `DuplicateSuccessors`: 11 generated for 2 distinct states). So the successor generator must reproduce TLC's enumeration, not just the set of successor states.
- **Successor order.** The order in which successors are produced decides which violation is found first, the counts at that point, and the counterexample (ADR 0015).
- **Evaluation errors.** These are verdicts, with TLC's error class. Laziness and evaluation order decide whether, and where, an error occurs.
- **Performance.** ADR 0006 targets:
  - at least 1.0× TLC on every benchmark spec at one worker, including `GameOfLife`, where tla-mc's interpreter is 0.24–0.29×;
  - at least 2× TLC as a geometric mean.

TLC paths are relative to `tlatools/org.lamport.tlatools/src/` at tlaplus `94d0c50`.

### What TLC does

**Evaluation.** `Tool.evalImpl`/`evalAppl` is a tree-walker over SANY nodes. Bindings live in a linked `Context`, and laziness uses `LazyValue` (ADR 0013, Context item 1).

**Successor generation.** `Tool.getNextStates` (`Tool.java:897-1680`) is a separate recursive enumerator:
- **Continuation and state.** It carries a continuation list (`ActionItemList`, whose items are a predicate, `UNCHANGED` or `CHANGED`) and a partially assigned successor `s1`, using bind and unbind.
- **Emission.** It emits each complete assignment to a functor (`Worker.addElement`, `Worker.java:418-491`). The functor counts the successor before checking it (`:420`).
- **The rules:**
  - Conjunction lists are consed in order. Disjuncts are tried left to right.
  - `\E` enumerates its bindings in normalized order with the **first** bound variable varying fastest:
    - `ContextEnumerator`; `Tool.java:4083-4116`;
    - probe `ExistsOrder`: after 11 comes 21, not 12;
    - probe `ExistsSetOrder`: normalized order.
  - `\A` becomes a list of conjuncts.
  - `x' = e` binds `x'` if it is unbound, and otherwise checks equality.
  - `x' \in S` enumerates `S` with `elements()` if `x'` is unbound, and otherwise checks membership (`:1422-1468`). Enumerated sets are normalized before they are iterated (`SetEnumValue.java:434-450`; probes `InSetOrder`, `FilterSetOrder`, `MapSetOrder`).
  - `x' \subseteq S` enumerates `SUBSET S`.
  - `UNCHANGED` handles a variable, a tuple or a zero-arity definition (`:1557-1680`). A variable already assigned to a different value is only a warning, `TLC_UNCHANGED_VARIABLE_CHANGED`.
  - `[A]_v` enumerates `A`, then the `UNCHANGED v` successor (`:1295-1305`; probe `StutterInNext`).
  - `<A>_v` adds a `CHANGED` check.
  - `IF`, `CASE`, `=>`, labels, `LET`, `SubstIn` and user-defined operators recurse.
  - Anything else is evaluated as a boolean.
- **The fast path.** Once every variable of `s1` is assigned, the remaining items are evaluated as booleans (`getNextStatesAllAssigned`, `:1047-1082`). It is on by default, because `TLCGlobals.warn = true` (`TLCGlobals.java:217`), and `-nowarning` turns it off (`TLC.java:516-519`).
- **Initial states.** `getInitStates` (`Tool.java:435-890`) does the same for initial states, with a functor that counts each one (`ModelChecker.java:1151-1155`).

**Choice and lazy sets.**
- **Bounded `CHOOSE`** normalizes the set and picks the first satisfying element (`Tool.java:2115-2200`). If no element satisfies it, or the set is not enumerable, the error is `GENERAL`.
- **Unbounded forms.** Unbounded `CHOOSE`, `\E` and `\A` always fail (`:2529-2549`).
- **Set filters** over a reducible set are computed eagerly, in the set's `elements()` order. Otherwise the result is a lazy `SetPredValue` (`:2469-2518`).
- **Set maps** are built eagerly by the context enumerator (`:2437-2449`).
- **Function constructors** are materialized when every domain is reducible and the context does not ask for laziness. Otherwise they stay lambdas (`:2354-2380`).
- **Recursive functions** stay lambdas and re-evaluate on each application (`FcnLambdaValue.java:91-260`).

**The TLC module.** `TLCGet` and `TLCSet` (`tlc2/module/TLCGetSet.java`) work as follows:
- **Registers.** Integer and `"s:"` registers are per worker thread (`IdThread.java:38-105`). A `TLCSet` on the main thread, as in `ASSUME` or `Init`, writes every worker's registers (`TLCGetSet.java:497-502`).
- **Live counters.** `"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"duration"`, `"stats"` and `"all"` read the checker's counters and registers while it runs (`:189-488`).
- **Per-state keys.** `"level"` is per state:
  - the current state's level in `Next`;
  - the successor's level in invariants and constraints;
  - 0 in `ASSUME` and `Init`.
- **Constant keys.**
  - `"config"` is built once, with `worker = TLCGlobals.getNumWorkers()` (`AbstractChecker.java:715-736`).
  - `"spec"` and `"revision"` are also run constants.
- **`TLCSet("exit", TRUE)`** finishes the queue and ends the run as a success (`:508-517`).

**Randomness.** In breadth-first search:
- the random generator is re-seeded once per expanded state, with `fingerprint(state) XOR seed` (`RandomEnumerableValues.java:136-166`);
- it starts from the plain seed during `Init`;
- trace reconstruction replays with the same seeding (`ConcurrentTLCTrace.java:138-144`; `Tool.java:3786`).

So `RandomElement` in `Next` is a function of the state, the seed and the fingerprint polynomial, never of the schedule.

**Java overrides.**
- **Kinds.** Overrides come in two kinds (`SpecProcessor.java:2114-2324`):
  - Value functions (`MethodValue`, `@TLAPlusOperator`) evaluate every argument first (`OpValue.java:53-64`).
  - Evaluating values (`EvaluatingValue`, `@Evaluation`) receive unevaluated arguments and the context. They can fall back to the TLA+ definition by returning null (`EvaluatingValue.java:83-95`).
- **Errors.** Any non-`EvalException` an override throws is reported as `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` (2154).
- **Discovery.** CommunityModules overrides are found through the class `tlc2.overrides.TLCOverrides` on the classpath. There is no service loader.

### Evidence on evaluator design

- **Closure tier.** tla-mc has an interpreter, a closure tier and a generated-Rust tier over one IR. Its closure tier measured **no gain** over its specialized interpreter (104.6 s against 101.2 s; tla-mc `docs/DESIGN.md:155-156`). Generated Rust gained 1.3–1.5×.
- **Where the wins came from:**
  - slot-based frames;
  - specializing boolean evaluation and reads of state paths by reference;
  - folding guards inside conjunctions;
  - continuation-passing enumeration over one reusable successor buffer (`eval.rs:740-867`; `closure.rs:782-795`).
- **Quint.** Quint's Rust evaluator compiles to closures (`docs/survey/adjacent.md`). Its source was not available to this review.
- **Matching TLC's counts.** tla2's interpreter mirrors TLC's continuation design and enforces generated-count parity as a release gate. Its parallel path counts at different points (after constraints) and diverges. tla-mc does not match TLC's generated counts, and says so (112.6 M against 115.3 M).

## Decision

### 1. A specialized plan, interpreted; closures and native code later, behind the same interface

**The evaluation plan.** `dive-eval` compiles the IR plus the `Model` into a plan: a compact, evaluator-private tree of specialized nodes.
- Slots are resolved to frame offsets.
- Constant-level definitions are replaced by their precomputed values.
- State-variable reads are typed (`VarRead(i)`, `PrimedRead(i)`).
- There are fast paths for common shapes: integer arithmetic and comparisons, `\in` on intervals and enumerated sets, record and function access by constant key, and boolean junctions that return `bool` without boxing.
- Every laziness point says whether it is lazy or strict (ADR 0013 §3).

**The interpreter.** In M5 the plan is executed by a match-dispatch interpreter. That is a deliberate choice:
- the measured evidence says dispatch style is not where the time goes;
- an interpreter is the simplest thing to make TLC-exact first.

**The `Evaluator` trait.** The plan sits behind an `Evaluator` trait (evaluate an expression, check a predicate, enumerate successors). A closure-compiling backend can implement it later, and is adopted only if it measures at least 1.3× over the interpreter on the benchmark set. `dive-compile` implements the same trait with generated Rust.

**Shadow comparison.** The interpreter stays the reference: every faster backend runs against it on the conformance corpus (ADR 0006).

### 2. Environments and laziness

**Frames and thunks.**
- Frames are vectors of slots.
- A lazy slot is a thunk: plan node, captured frame, and a once-cell holding the result or the error.
- Thunks are created exactly where TLC creates `LazyValue`s:
  - user-operator arguments;
  - zero-arity `LET` definitions, once per evaluation of the `LET`;
  - `Subst` bindings.

**Reusing a forced thunk.** A forced thunk is reused exactly when TLC's `LazyValue.getCachedValue` would reuse its value:
- the predecessor state is the same;
- the successor agrees on every variable assigned when the thunk was forced;
- the evaluation mode (`Primed`, `Enabled`) is equivalent.

Otherwise it is re-evaluated. A generation counter on the successor buffer makes the common case one comparison.

**Strict points.** Strict points are evaluated eagerly. This is only allowed when the effects analysis proves the expression is pure and cannot fail.

**Standard-module arguments.** Arguments to value-function overrides (`StdOp` of the value kind) are evaluated eagerly and left to right, as `OpValue.eval` does.

### 3. Successor generation is a port of TLC's enumerator, in its own module

**The module.** `dive_eval::next` ports `getInitStates` and `getNextStates` rule by rule. Their Java line ranges are cited in the code, and each rule has a probe or a unit test.
- It works in continuation-passing style: an explicit continuation list, the same as `ActionItemList`.
- It uses one reusable successor buffer per worker, with bind and undo.
- It emits each complete assignment to a sink, `FnMut(&Successor) -> Flow`.

**What it reproduces.** Every Context rule, including:
- the all-assigned fast path. It is on by default; `--no-warnings` turns it off, as `-nowarning` does;
- argument order and source order of junction items;
- normalized, first-variable-fastest enumeration of `\E`;
- `elements()` order for `x' \in S`;
- the stuttering successor of `[A]_v`.

**What the sink is told.** For each emitted successor:
- its ordinal within the expanded state;
- the action that produced it (id and label);
- whether every variable is assigned.

The checker (ADR 0015) counts, constrains, deduplicates and checks.

**What the enumerator does not do.** It never deduplicates, filters or reorders successors. "States generated" is the number of sink calls, exactly as `Worker.addElement` counts them.

**Errors.** An evaluation error ends the enumeration of that state. The error carries the number of successors already emitted, which ADR 0015 needs for counting.

**Kept apart from the trace validator.** The enumerator sits behind a cargo feature, `enumerate`, on by default. `dive-trace` depends on `dive-eval` with that feature off, so the independent trace validator cannot call it (ADR 0016).

### 4. Lazy sets

`dive-value` owns set representations, including the `DeferredSet` hook for evaluator-driven filters and maps (ADR 0012). The evaluator decides representations exactly where TLC does:

| Construct | Representation |
|---|---|
| `{x \in S : P}` | eager, in `S`'s enumeration order, if `S` is finite and enumerable; otherwise a deferred filter |
| `{e : x \in S}` | eager, by the context enumerator |
| `[x \in S \|-> e]` | materialized if every domain is finite and enumerable and the context is not `KeepLazy`; otherwise a lambda |
| `f[x]` | evaluates `f` in `KeepLazy` mode |
| `SUBSET S`, `[S -> T]`, `S \X T`, `[a : S]`, intervals, `Nat`, `Int`, `STRING`, `Seq(S)` | lazy; membership without enumeration; enumeration on demand, once per set object, in the order `dive-value` defines for each kind |

Infinite sets refuse enumeration with TLC's message.

An enumeration-order probe for each lazy kind (`SUBSET`, `[S -> T]`, `\X`, record sets) is part of WP-C4 in the M5 plan. The `SetEnum` probes show normalized order; TLC's code suggests generation order and normalized order coincide for the other kinds, but that must be measured, not assumed.

### 5. `CHOOSE` and value order

- **Order.** `CHOOSE x \in S : P` walks `S` in the run's `ValueOrder` (`dive-value`'s `Ctx`) and returns the first element that satisfies `P`. The order is TLC's intern order under `--choose-order=tlc` (part of `--compat=tlc`) and canonical order by default (ADR 0010).
- **Errors.** Every `CHOOSE` failure and every unbounded form raises TLC's error in both profiles.
- **The same order drives every other enumeration.** `\E`, `x' \in S`, filters and maps all follow the `ValueOrder`. That is what makes `--compat=tlc` reproduce TLC's successor order. In the default profile, it means the order of exploration among strings and model values is canonical too. ADR 0015 §7 records what that changes and asks the owner how to treat it.

### 6. Integers

- **The mode.** `dive-value`'s `IntMode` is part of the `Ctx`:
  - `Big` (default): an inline `i64` fast path that spills to a big integer;
  - `Tlc`: 32-bit range checks raising `TLC_MODULE_OVERFLOW` (2178), and `TLC_INTEGER_TOO_BIG` for literals.
- **Reporting a difference.** In `Big` mode, the first arithmetic result outside the 32-bit range is recorded with its origin. The output's `options` block then says the run would differ under `--compat=tlc` (ADR 0010 §3). Only the slow path pays for the check.

### 7. The TLC module in `dive-stdlib`, and how parallel search sees it

**Registers.**
- dive keeps **one logical register file** per run. `TLCSet` in `ASSUME` or `Init` writes it, as TLC's main thread writes every worker's registers.
- A spec whose checking path can write a register or read a live counter is checked in **sequential mode** (ADR 0015 §6). This covers anything reachable from `Init`, `Next`, invariants, constraints, action constraints, implied actions or `VIEW` that:
  - calls `TLCSet`;
  - reads an integer or `"s:"` register;
  - reads `TLCGet` of `"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"duration"`, `"stats"`, `"all"` or `"all:named"`.

  In sequential mode these return exactly the values TLC's single worker sees. A register read with no write in the checking path is a run constant and does not force sequential mode.

**`TLCGet` keys.**
- **`"config"`** has TLC's fields:
  - `mode = "bfs"`, `deadlock`;
  - `seed`, the reported seed;
  - `fingerprint`, the polynomial of the reported index;
  - `install`;
  - `worker`, which reports the **logical** worker count, 1. dive's search at any worker count is, by construction, TLC's single-worker search (ADR 0015), and the harness holds dive to the single-worker golden. Examples `MCDieHardest` asserts `worker = 1`. The physical count is in dive's own output. This resolves the open design point in `docs/divergences.md`, subject to owner confirmation (open question 10).
- **`"stats".worker`** is 0.
- **`"revision"`** returns dive's own version. Results that depend on it are flagged, like any introspection.

**`TLCSet("exit", TRUE)`** ends the run as a success after the current state. It also forces sequential mode.

**`Print` and `PrintT`.** Output is buffered per expanded state and released in TLC's sequential order, so the output is the same at every worker count. At a stop, only output up to the stopping event is released (ADR 0015 §3).

**`Assert`** raises `TLC_VALUE_ASSERT_FAILED` (2132), exit 14 (harness fixture `AssertFailure`).

**`RandomElement` and `Randomization`.** These follow TLC's seeding structure:
- one generator per expanded state, seeded from the state's fingerprint and the run seed;
- the plain seed during `Init`.

They are deterministic per state, so they do not force sequential mode.
- **Under `--compat=tlc`**, reproducing TLC's draws needs three things:
  - Java's `java.util.Random` sequence;
  - TLC-identical state fingerprints, which depend on TLC's variable order: the reversed enumeration order of a Java `Hashtable` over the root module's whole context (`TLCStateMut.java:250-254`; `tla2sany/semantic/ModuleNode.java:400-408`; `Context.java:134`, `:320-331`);
  - TLC's per-value FP64 encoding, which `dive-value` already has.

  Until the variable-order emulation lands (WP-A3), a spec that draws random values while checking is refused under `--compat=tlc` as `unsupported`, naming the feature.
- **In the default profile**, dive uses its own state fingerprint for seeding. The run is replayable from the reported seed at any worker count (D-002).

**`JavaTime`** is nondeterministic, as in TLC. It is marked impure and flagged in the output.

**Mapping to Java overrides.**
- `dive-stdlib` implements every `StdOp` (ADR 0013 §5) as either a value function or an evaluating function, mirroring TLC's two kinds.
- Each implementation's rustdoc cites the Java method it matches (for example `tlc2/module/Sequences.java`, `Head`), with provenance recorded in `docs/provenance.md`.
- An evaluating function may return `Fallback`, meaning "evaluate the TLA+ definition", only where TLC's returns null.
- `dive-eval` calls `dive-stdlib` through a table of function pointers, which `dive-model` installs. There is no dynamic dispatch per call beyond the indirect call.
- CommunityModules operators are implemented on demand, in order of corpus use. An unimplemented one is refused by name.

### 8. Errors

**What an error carries.** Evaluation errors are values, never panics:
- TLC's `EC` number and name;
- TLC's message text and parameters (`dive-value`'s errors already use TLC's wording);
- the origin;
- the stack of enclosing origins that TLC prints as `TLC_NESTED_EXPRESSION`;
- the state context.

**Phase codes.** The checker adds the code TLC uses for the phase:
- `TLC_INVARIANT_EVALUATION_FAILED`;
- `TLC_ACTION_PROPERTY_EVALUATION_FAILED`;
- `TLC_INITIAL_STATE` for an illegal initial state;
- `TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT` (2109) for a successor with an unassigned variable;
- `GENERAL`, followed by `TLC_FINGERPRINT_EXCEPTION` or `TLC_NESTED_EXPRESSION`, for errors while expanding a state, as TLC's call-stack replay reports them (harness fixture `EvalError`).

**The error-class table.**
- The exact mapping from error site to the class the harness compares is a table built from captured probes (WP-C4). It is not guessed.
- An error whose class dive cannot yet reproduce is still reported as an evaluation error. The harness then records a disagreement on the class, never an agreement.

**Never swallowed.** Errors are never swallowed or counted and skipped (ADR 0006). The one exception is TLC's own: a constant definition whose start-up evaluation fails stays lazy (`SpecProcessor.java:373-380`), and dive does the same.

**Depth.** Evaluation depth is bounded explicitly: deep recursion, deeply nested values, long `LET` chains.
- Workers run on threads with large stacks.
- A depth counter turns exhaustion into `resource-limit`, naming the construct, instead of a stack overflow.

TLC's equivalent outcome is a JVM `StackOverflowError`, which the harness classifies as `resource-limit` and never makes a golden (ADR 0003).

## Consequences

- Generated-state counts and successor order are reproducible because the enumerator is a rule-by-rule port, separate from expression evaluation. It is also the part most likely to drift when the TLC pin moves, so every rule carries its Java citation and a probe.
- Starting with an interpreter makes dive's first verdicts depend on getting semantics right, not on a code generator. The performance targets are then met by measurement:
  - the plan's fast paths;
  - allocation discipline (ADR 0015 §8);
  - an optional closure backend;
  - eventually `dive-compile`.
- Sequential mode for register-writing and counter-reading specs keeps them TLC-exact at the cost of parallelism. They are rare in the corpus (Examples `MCDieHardest` reads `"config"` only, which does not force it). The output always says when it happened.
- `RandomElement` under `--compat=tlc` waits for TLC variable-order emulation, an honest refusal until then.
- Revisit:
  - after the first benchmark round, for whether to build the closure backend;
  - when liveness needs `ENABLED` beyond safety checking;
  - if the error-class table shows TLC behaviour that the phase-code model cannot express.

## Amendments

**2026-10-08, WP-C1 (the interpreter).** Building the interpreter over the IR as lowered (ADR 0013's amendments) and checking it against the pinned TLC on 217 fixture specifications and the corpus `ASSUME`s settled these points. The sections above are unchanged; where they differ, this amendment holds.

1. **Two scopes, not one (§2).** TLC binds every name in one linked `Context`, looked up by semantic node, and an operator body is evaluated in its *caller's* context (`getOpContext` conses the parameters onto `c`). Parameters, bound variables and `LET` values are nevertheless effectively lexical, because their nodes are unique. Instance substitutions and `@` are not: `I!Op` calls definitions of the instantiated module that SANY does not copy, and their references to the module's declarations must find `I`'s substitutions in the caller's context (`Tool.evalImplSubstInKind`, `SymbolNodeValueLookupProvider.lookup`). dive's environment is therefore a lexical frame of slots plus a dynamic part (the substitution list and `@`), and a callee inherits the caller's dynamic part. Fixture `EvInstance` (`L(n) == INSTANCE Inner WITH K <- n * 100`, `L(3)!Double`, `L(2)!Both`) agrees with TLC.
2. **Captures snapshot their frame (§2).** A thunk, a lazy set or a lazy function captures its environment, as TLC's `LazyValue`, `SetPredValue` and `FcnLambdaValue` capture their `Context`. Frames are copy-on-write (`Arc::make_mut`): a quantifier that rebinds its variable clones the frame only when something has captured it, so an earlier capture never sees a later binding.
3. **Variables are never strict.** As ADR 0013's amendment 2 says, an argument naming a variable stays a thunk; the interpreter forces it where TLC would, and an unassigned variable raises `TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR` there.
4. **Lazy functions are evaluator values.** `dive-value` has no lambda representation, so a function TLC keeps as a `FcnLambdaValue` (a recursive function, a non-enumerable domain, or `KeepLazy`, which TLC passes down through the function position of `f[x]`) is an evaluator value. It is applied, takes `EXCEPT`s and gives its `DOMAIN` without being computed. Where TLC stores such a lambda inside another value, dive computes it only when that is indistinguishable (the body is pure and computing it succeeds) and refuses otherwise.
5. **Java overrides wrap errors (§7).** A value override (`MethodValue`) passes an `EvalException` through, but wraps any other exception, including TLC's own `Assert.fail` errors raised in the override, in `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` with the Java method's signature (`MethodValue.eval`). A non-integer argument of an override declared `(IntValue, IntValue)` fails in the method handle's spreader with `Cannot cast tlc2.value.impl.StringValue to tlc2.value.impl.IntValue`. dive reproduces both (fixtures `ErPlusNonInt`, `ErCardIncomparable`, `ErrCardSubset`).
6. **Which overrides apply depends on the invocation.** The harness runs TLC with `-dumpTrace json`, which makes TLC load `_JsonTrace`, which loads `Integers`. `Integers`' overrides then replace `Naturals`' even in a specification that extends only `Naturals`: `"a" >= 1` reports `>=`, not `Naturals.GEQ`'s `>`, and `-2147483648 \div -1` overflows (fixtures `ErGeqNaturals`, `ErPlusNonIntNaturals`, `RvNatDiv`). Selecting the overrides is the binding stage's job (WP-A1/B1); the evaluator runs whichever `StdOp` the IR names.
7. **Literal errors happen at start-up.** A numeral too big for `--int=tlc` (`TLC_INTEGER_TOO_BIG`) and a decimal (`TLC_CANT_HANDLE_REAL_NUMBERS`) stop TLC while it processes the specification, before any assumption (fixtures `OvLit`, `ErDecimal`). `Evaluator::startup_error` reports them for the binding stage.
8. **Two pending owner questions are options.** Exploration order (open question 9) is `Options::exploration`, and comparisons (open question 12, D-009) are `Options::comparisons`. Both default to TLC's behaviour in both profiles until the owner answers; each answer is a one-line change to `Options::dive`.
