# Successor generation

Status: M0 spike implemented in `crates/dive-core/src/generate.rs` on
`core/m0-spike`, for the constructs of the spike models. This document
specifies how TLC enumerates initial states and successors. The new core must
reproduce that enumeration exactly. The document also covers how the new core
generates, hashes and deduplicates successors without changing any count.

**Citations.** TLC citations are relative to
`tlatools/org.lamport.tlatools/src/` of the pinned tlaplus commit `94d0c50`
(the oracle `tlc-2026.10.06.014338`). Line numbers refer to that commit. dive
citations refer to the reference engine, which is frozen apart from bug fixes
(`README.md`).

## 1. Scope

This document fixes two observables:

- **Generated counts at 1 worker.** TLC's "states generated" is the number of
  initial states handed to `DoInitFunctor.addElement`, plus the number of
  successors handed to `Worker.addElement`. Both functors count first and
  check afterwards: `numberOfInitialStates++` at `tlc2/tool/ModelChecker.java:1155`
  and `this.statesGenerated++` at `tlc2/tool/Worker.java:420` come before the
  completeness, constraint, seen-set and invariant checks. The total is summed
  in `ModelChecker.getStatesGenerated` (`ModelChecker.java:1028-1035`). Every
  emitted state therefore counts, including duplicates, states outside the
  model and states with unassigned variables.
- **The order of the successors.** For each state, the enumeration produces a
  sequence of successors. Its index in that sequence, across all actions in
  action order, is the successor's *ordinal*.

Order is observable, not only counts:

1. **Counts at a stop.** When a successor decides the run (a violation, an
   incomplete state or an evaluation error), the counts include everything
   emitted up to and including it, and nothing after it (probe
   `MidLevelViolation` in `crates/dive-check/tests/fixtures/tlc-probes/`). A
   different order gives different counts.
2. **Which violation is reported.** When two successors at the same depth
   violate different invariants, the one emitted first is reported (probes
   `ExistsOrder`, `StringOrderViolation`; rule fixture `NxForallOrder`).
3. **Trace reconstruction.** The trace log records only `(parent, ordinal)` for
   each new state. A counterexample is rebuilt by regenerating each parent's
   successors and taking the recorded ordinal. This is
   `crates/dive-check/src/engine.rs` `replay` (around :2072) and, for TLC's
   call-stack replay after an invariant error, `replay_after` (around :1720).
   If the replay regenerates a different state, the result is an internal
   error (module documentation of `engine.rs`, around :30). A core that logs
   ordinals and a replayer that disagree on the order cannot produce traces.
4. **Deadlock.** A state is deadlocked when expanding it emits no successor at
   all (`Worker.java:114-125` compares `statesGenerated` before and after).
   Successors that are seen, or that fail a constraint, still count. That is
   why they prevent deadlock (probes `ConstraintNoDeadlock`, `StutterInNext`).

**Profiles.** The rules below hold in both profiles. The profiles differ in
three ways, and each is a registered divergence:

- the value order used to normalize sets: TLC's order under `--compat=tlc`,
  canonical order by default (D-010, `ExplorationOrder`);
- action composition `\cdot` (D-022);
- the warning switch, described in section 5.

The enumeration structure, and therefore the complete-run generated count, is
the same under D-010. The counts at a stop and the reported violation can
differ.

**Out of scope.** Simulation's randomized branches (`PROBABILISTIC` in
`Tool.java`) are out of scope. So are `ENABLED`, which uses its own search over
the same rules (`crates/dive-eval/src/next.rs`, ENABLED section from :2069), and
the detailed semantics of `\cdot`.

## 2. Action splitting

TLC does not enumerate `Next` as one formula. At start-up it splits `Next` into
a flat, ordered list of actions (`Tool.getActions`, `Tool.java:273-419`). It
then numbers them after the initial predicates (`Tool.java:237-241`). Each
`Action` holds a predicate, a context and a name (`tlc2/tool/Action.java`). The
name and the context's parameter values make up the action label in traces
(`Action.getLocation`, `Action.java:101-117`). Splitting walks the formula as
follows:

```text
split(e, ctx, name):
  case e of
    LET defs IN b        -> split(b, ctx, name)          -- defs are not bound
    label :: b           -> split(b, ctx, name)
    SubstIn [] b         -> split(b, ctx, name)
    SubstIn subs b       -> push Action(e, ctx, name)    -- with substitutions: one action
    A1 \/ ... \/ An      -> for i in 1..n: split(Ai, ctx, name)
    \E bounds : b        -> mark := |out|
                            try
                              en := contexts(e, ctx, Empty, Empty, Clear)   -- normalized
                              if en is empty: push Action(e, ctx, name)
                              else for c in en: split(b, c, name)
                            catch any:
                              truncate out to mark; push Action(e, ctx, name)
    Op(a1..an), Op a user definition, every ai of constant level
                         -> try
                              ctx' := ctx + {param_i |-> eval(ai, ctx, Empty)}  -- values, not lazies
                              split(body(Op), ctx', Op)
                            catch any:
                              push Action(e, ctx, Op)    -- actions already added stay
    anything else        -> push Action(e, ctx, name)
```

TLC sources for each case:

- LET, label and substitution: `getActions`, `Tool.java:281-324`.
- Inlining a user operator: `getActionsAppl`, `Tool.java:337-372`. The
  `catch (Throwable e) { /*SKIP*/ }` at `:364` keeps the actions the body had
  already pushed, then adds one more action.
- Bounded `\E`: `Tool.java:375-402`, including `isDone()` for an empty domain
  and `removeAll(cnt)` on an exception.
- Disjunction: `Tool.java:403-410`.

Everything else becomes one action. That includes a conjunction, `IF`, `[A]_v`,
`UNCHANGED` and an application whose arguments are not all constant. An
operator that TLC overrides in Java is one action named after the override. An
operator whose lookup is neither a definition nor itself a definition throws
`ClassCastException`. The nearest enclosing inlining or `\E` catches that
exception.

**dive.** `crates/dive-ir/src/actions.rs` `split` (:140) and
`split_with_values` (:173) build an `ActionTree` (`Splitter::tree`, :232) that
records every point where TLC's walk branches on a value or an exception. The
`Throws` and `Configured` nodes stand for the exception and
configuration-dependent cases.

The tree is expanded with values by `Expander::expand` and `Expander::inline`
(`crates/dive-eval/src/next.rs` :1878 and :1994). The expansion is driven by
`crates/dive-model/src/actions.rs`:

- `expand_actions` (:68) adds configuration redirects and start-up values. It
  also makes a next-state relation found under instance substitutions into
  one action.
- `init_actions` (:52) numbers the initial predicates from 0.

`crates/dive-ir/tests/actions.rs` compares every action's name, parameters,
label, order and count with TLC's own action list, using fixtures
`SplitBasic`, `SplitDomains`, `SplitLet`, `SplitOverride`, `SplitSpec`,
`SplitSubst` and `SplitThrows`.

**Why splitting matters.** Splitting decides action names and labels, the
context each action runs in (inlined parameters are values, not lazy values),
and where an error surfaces. It is not meant to change the successor sequence.
For each state, the concatenation of the per-action sequences in action order
is the sequence the engine numbers. This document does not prove that
splitting never changes the sequence. The new core must reproduce the split
exactly rather than rely on that property.

## 3. Initial-state enumeration

`Tool.getInitStates(IStateFunctor)` (`Tool.java:435-454`) enumerates the first
initial predicate. It passes the other predicates, in order, as the
continuation. Because they are consed from the tail, they are evaluated in
order. The comment at `:438-441` records the earlier bug that evaluated them in
reverse.

The partial state `ps` starts with every variable unassigned. The rules are
those of section 4, with these differences (`getInitStates`, `:473-525`;
`getInitStates(acts, ...)`, `:527-572`; `getInitStatesAppl`, `:574-890`):

- **Assignments name unprimed variables.** `x = e`, `x \in S` and
  `x \subseteq S` resolve `x` with `getVar` and require `varLoc >= 0`
  (`:756-853`). Otherwise the conjunct is evaluated as a boolean.
- **Evaluation runs against `ps`.** Each evaluation is
  `eval(e, c, ps, TLCState.Empty, EvalControl.Init)`: the partial state is the
  current state and there is no next state.
- **The all-assigned shortcut is unconditional.** Once every variable of `ps`
  is assigned, the rest of the continuation is evaluated as booleans
  (`:537-568`), whatever the warning switch is (fixture `NxInitFastPath`).
- **Some constructs have no rule of their own.** `UNCHANGED`, `[A]_v`,
  `<<A>>_v` and `\cdot` fall through to the default boolean evaluation
  (`:875-888`).
- **A lazy value's cache is checked against `ps`.** `LazyValue` caches are
  consulted with `(ps, null, Clear)` (`:611-618`).
- **Error texts differ.** They read "In computing initial states, ...".

Every state that reaches an empty continuation is emitted and counted,
duplicates included (`NxInitDup`: `x = 0 \/ x = 0 \/ x = 1` generates 3 initial
states, 2 distinct). An emitted initial state with an unassigned variable is
counted first. It is then reported as `TLC_INITIAL_STATE` (2102), "current
state is not a legal state" (`ModelChecker.java:1170-1174`).

dive: `Evaluator::init_states` (`next.rs` :439) runs the same `Gen` with
`init: true`. `Gen::st` (:651) builds the `(ps, Empty, Init)` evaluation states.
`Gen::st_cache` (:670) builds the cache-check states. `init_all_assigned`
(:773) is the unconditional shortcut.

## 4. Successor-enumeration rules

### 4.0 The machine

For one state `s0` and one action, TLC runs a continuation-passing search over
these parts:

- **`s1`.** A mutable partial state with one slot per variable; `null` means
  unassigned (`TLCStateMut`, `tlc2/tool/TLCStateMut.java:103-118`).
- **`acts`.** A persistent list of pending items (`ActionItemList`). Each item
  has a kind, an expression and a context. The kinds are defined in
  `tlc2/tool/IActionItemList.java:37-49`. A conjunct carries its positive
  index. `PRED` is -1, `UNCHANGED` is -2 and `CHANGED` is -3. Conjuncts and
  `PRED` items are treated alike.

```text
gen(e, acts, ctx)              -- Tool.getNextStates(action, pred, acts, c, s0, s1, nss)
cont(acts)                     -- Tool.getNextStates0, Tool.java:1001-1045
  if acts is empty:            emit(s1)              -- counted, complete or not
  elif warn and all_assigned(s1): fast(acts)        -- section 5
  else (k, e, c) := head(acts); rest := tail(acts)
       k is conjunct or PRED:  gen(e, rest, c)
       k is UNCHANGED:         unchanged(e, rest, c)
       k is CHANGED:           if eval(e, c, s0) != eval(e, c, s1): cont(rest)

fast(acts)                     -- getNextStatesAllAssigned, Tool.java:1047-1083
  for (k, e, c) in acts:
    conjunct or PRED: b := eval(e, c, s0, s1, Clear)
                      non-boolean -> error; FALSE -> return
    UNCHANGED:        return unchanged(e, rest, c)   -- leaves the shortcut
    CHANGED:          if values equal -> return
  emit(s1)
```

**State discipline.** Each binding rule binds a slot, runs its continuation and
unbinds the slot. `emit` hands `s1` to the functor and returns `s1.copy()`
(`Tool.java:1004-1005`). Later branches continue on that copy, with the same
bindings. The observable effect is that of one backtracking buffer: each
successor is a snapshot of the buffer at the moment it is emitted.

**Dispatch.** `getNextStatesImpl` (`:928-963`) handles `LET`, `SubstIn`,
`APSubstIn` and labels. `getNextStatesApplImpl` (`:1102-1157`) handles operator
applications. `getNextStatesApplSwitch` (`:1185-1552`) dispatches built-in
operators by opcode.

**dive.** The same machine is `Gen` in `crates/dive-eval/src/next.rs`:

- `Kind`, `Item`, `Acts` and `cons` (:576-607) are TLC's continuation list.
- `Gen::cont` (:728) is `getNextStates0`, and `next_all_assigned` (:788) is the
  shortcut.
- `Gen::step` (:857) dispatches.
- `Gen::emit` (:706) numbers successors with `ordinal`.
- `Evaluator::next_states` (:488) runs one action.
- `Evaluator::successors` (:522) runs every action, threading the running
  ordinal across them.

### 4.1 Conjunction, as a continuation

For `A1 /\ ... /\ An`, TLC conses `A2..An` from the back, so `A2` ends up at
the head, and then runs `gen(A1, acts', ctx)`. Conjuncts are therefore
processed left to right, each in the context of the conjunction (`:1189-1197`).
dive: `step`, `JunctionKind::And` (:863-873).

No successor is emitted here. After the conjunct that assigns the last
variable, the remaining conjuncts are popped by `cont`. Under the shortcut,
they are then evaluated as booleans and are no longer enumerated (`NxFastPath`).

### 4.2 Disjunction

TLC runs each disjunct in order, with the same `acts` and the same partial
state (`:1198-1219`). dive: `step`, `JunctionKind::Or` (:874-882).

Each disjunct's emissions count, so `x' = 1 \/ x' = 1` generates 2
(`DuplicateSuccessors`). A disjunction that is popped when every variable is
already assigned goes through the shortcut and counts as one boolean test, not
as several branches.

### 4.3 Bounded `\E` and `\A`

**`\E`** (`:1220-1244`). `contexts` (`:4083-4113`) evaluates every domain
first, in `(s0, s1, Clear)`, and fails if a domain is not enumerable. It
enumerates each domain with `elements(Ordering.NORMALIZED)`. A variable list
`x, y \in S` gets one enumerator per variable. A tuple binder `<<a, b>> \in S`
gets one enumerator, and a non-matching element is `TLC_ARGUMENT_MISMATCH`.

`ContextEnumerator` (`tlc2/tool/impl/ContextEnumerator.java:24-73`) works like
an odometer, with the **first** position fastest. If any domain is empty, the
enumerator is done at construction and generates nothing. For each context,
TLC runs `gen(body, acts, c)`.

dive: `Core::for_each_context` (`crates/dive-eval/src/eval.rs` :1281) uses the
same odometer, first position fastest. It returns immediately when any column
is empty, and only interval domains are streamed. The caller is `step`
(:884-911).

**`\A`** (`:1245-1262`). TLC enumerates the contexts the same way:

- With no context, the result is `cont(acts)` (`NxForallEmpty`).
- Otherwise TLC conses the body under each later context `c2, c3, ..., cn` onto
  `acts` in that order. The list then holds `cn` at the head and `c2` last. TLC
  then runs `gen(body, acts', c1)`.

The bindings are therefore processed in the order `c1, cn, cn-1, ..., c2`. In
`NxForallOrder` (3 generated, `Inv` violated), `x' = i \/ TRUE` over
`{1, 2, 3}` emits `x = 1` and then `x = 3`, not `x = 2`. dive: `step`
(:912-932) collects the contexts and conses them the same way.

### 4.4 IF, CASE and `=>`

The guard is evaluated in `(s0, s1, Clear)`, with whatever `s1` holds at that
point. A guard that reads an unassigned primed variable is an evaluation error.

- **`IF g THEN a ELSE b`** (`:1306-1320`): run `gen(a)` or `gen(b)`. A
  non-boolean guard is an error with TLC's text.
- **`CASE`** (`:1321-1352`): the arms are tested in order, and the first true
  guard's value is run. Without a true guard, `OTHER` is run. Without `OTHER`,
  the result is the error "TLC encountered a CASE with no conditions true".
- **`P => Q`** (`:1470-1483`): if `P` is false, `cont(acts)` runs and `Q` is
  not enumerated. If `P` is true, `gen(Q)` runs.

dive: `step` (:933-1000). `NxIfCase` generates 8 states, 7 distinct.

### 4.5 `x' = e`, `x' \in S` and `x' \subseteq S`

The left side is resolved with `getPrimedVar` (`tlc2/tool/impl/Spec.java:138`).
It sees through `SubstIn` and `APSubstIn` (their substitutions bound as
uncached lazy values), `LET` (zero-arity definitions bound lazily) and labels.
`v'` resolves to `getVar(v)`. A zero-arity name is followed: a definition to its
body, a `LazyValue` to its expression in its own context
(`SymbolNodeValueLookupProvider.getVar`, `:29`).

If the left side is not a primed variable, the whole formula is evaluated as a
boolean. Otherwise, `r` is evaluated first, whether or not `x'` is already
assigned:

```text
x' = e        (:1353-1378)  r := eval(e)
                            unassigned: bind x := r; cont(acts); unbind x
                            assigned:   if s1[x] equals r: cont(acts)
x' \in S      (:1422-1469)  r := eval(S)
                            unassigned: require Enumerable, else "the right side of \IN is not enumerable"
                                        for v in r.elements():          -- unnormalized; section 6
                                          bind x := v; cont(acts); unbind x
                            assigned:   if r.member(s1[x]): cont(acts)
x' \subseteq S (:1379-1421) r := SUBSET eval(S)
                            unassigned: for v in r.elements(): ...      -- normalized
                            assigned:   if r.member(s1[x]): cont(acts)
```

`equals` and `member` are TLC's value operations, and they can raise errors,
for example when comparing values of different kinds. Those errors are part of
the contract. One generated successor results for each binding whose
continuation reaches the end of the list. `x' \in {0, 1} /\ x' = 1` generates 1
(`DuplicateSuccessors`). `NxSubseteq` generates 17 states, 4 distinct.

dive: `Gen::assign` (:1043) with `Core::primed_var` (:1588) and `var_lookup`
(:1600). The right side of `=` is forced with `embed`, so a lazy function
becomes a value. Intervals are bound in ascending order without being built
(`bind_interval`, :1181). Other sets are streamed or listed in TLC's
`elements()` order (`in_element_stream`, `in_elements`, `tlc_enum.rs` :185 and
:262).

### 4.6 UNCHANGED and tuples of variables

`processUnchanged(e, acts)` (`processUnchangedImpl`, `:1560-1588`) works as
follows:

- **`e` is a variable** (`getVar`, `processUnchangedImplVar`, `:1652-1679`).
  Let `v0 := s0[x]`. If `x'` is unassigned, bind `x := v0`, run
  `cont(acts)` and unbind. If it is assigned and equal to `v0`, run
  `cont(acts)`. Otherwise nothing is generated, and TLC prints warning
  `TLC_UNCHANGED_VARIABLE_CHANGED` (2143) (`:1676`). `MP.printWarning`
  prints only while `TLCGlobals.warn` is on, and only once per distinct
  message text (`tlc2/output/MP.java:1897-1919`).
- **`e` is a tuple `<<e1, ..., en>>`** (`:1638-1649`). TLC conses `e2..en` as
  `UNCHANGED` items from the back and processes `e1`. An empty tuple is
  `cont(acts)`.
- **`e` is a zero-arity operator** (`:1591-1604`). If the lookup gives a
  definition, TLC processes its body. If it gives a `LazyValue`, TLC processes
  the lazy value's expression in its context. Anything else is verified.
- **Anything else** (`verifyUnchanged`, `:1610-1620`). `e` is evaluated in `s0`
  and in `s1` (taken as the current state, with no next state). If the two
  values are equal, `cont(acts)` runs.

The shortcut changes which warnings appear. A `UNCHANGED` that is an ordinary
conjunct, popped when every variable is assigned, is evaluated as a boolean.
In that case no warning is printed (`Tool.java:1007-1015`). Only
`UNCHANGED`-kind items, the later elements of a tuple, leave the shortcut.

In `NxUnchangedChanged` (3 generated, 3 distinct):

- Action `A` is `x' = 1 /\ y' = 0 /\ UNCHANGED <<y, x>>`. It reaches its
  `UNCHANGED` with every variable assigned, so it fails silently.
- Action `B` is `x' = 2 /\ UNCHANGED <<x, y>> /\ FALSE`. It reaches its
  `UNCHANGED` with `y` unassigned and prints the only warning, at line 9.

dive: `Gen::unchanged` (:1210), `verify_unchanged` (:1256) and
`Core::zero_arity_target` (:1522).

### 4.7 `[A]_v` and `<<A>>_v`

- **`[A]_v`** (`:1295-1305`). TLC runs `gen(A, acts)`, then
  `processUnchanged(v, acts)`, with the same `acts` and partial state. The
  stuttering successor comes after `A`'s successors. If an `A` step leaves `v`
  unchanged, that step is generated twice.
- **`<<A>>_v`** (`:1290-1294`). TLC conses a `CHANGED v` item and runs
  `gen(A)`. The item is checked after `A`'s own conjuncts. It compares `v`
  evaluated in `s0` and in `s1`.

`NxStepAngle` generates 10 states, 4 distinct:

- from `x = 0` and `x = 1`: one `<<A>>_x` successor, then `B`'s step, then
  `B`'s stutter;
- from `x = 2`: one successor and the stutter;
- from `x = 3`: only the stutter.

Probe `StutterInNext` gives 6 generated and 3 distinct. dive: `step`, arms
`StepBox` and `StepAngle` (:1007-1018), and `Gen::changed` (:755).

### 4.8 LET, substitution and label transparency

- **`LET`** (`:937-941`). TLC continues with the body and binds nothing. A use
  of a `LET` name is looked up and found as a definition (`OpDefNode`), so its
  body is enumerated afresh at every use. In `NxLetAction`, `LET A == x' = x + 1
  IN A \/ A` generates 2 successors per state (5 generated, 3 distinct). dive
  gets the same effect by binding the `LET` values as *uncached* thunks
  (`Core::bind_let_uncached`, :1692).
- **`SubstIn`** and **`APSubstIn`** (`:966-987`). TLC binds each substitution
  as `getVal(expr, c, false, ...)`, an uncached `LazyValue` over the outer
  context. An instance action therefore assigns the variable that its own
  variable is substituted by (`NxInstance`: 4 generated, 4 distinct). dive:
  `Core::bind_subst(..., false)` (`crates/dive-eval/src/scope.rs` :1339),
  called from `step` (:1025-1029).
- **Labels and `Op!1` subexpression names** (`OPCODE_nop`, `:1534-1537`) are
  transparent. dive: `step` (:1030).

### 4.9 User-operator application: inlined or evaluated

When an application `Op(args)` has no built-in opcode, TLC looks `Op` up
(`:1110-1154`) and acts on the result:

- **A definition (`OpDefNode`) or theorem/assumption node.** TLC runs
  `gen(body, acts, getOpContext(args, cachable = true))`. Each argument is
  bound as a *cached* `LazyValue` over the caller's context
  (`SymbolNodeValueLookupProvider.java:124-147`). An argument that names a
  variable lets the body assign it. In `NxLazyArgVar`, `F(x)` assigns `x` in
  `Init` and `G(x)` assigns `x'` in `Next` (5 generated, 4 distinct).
- **A `LazyValue`**, for example a formal parameter applied as an action.
  TLC consults `lv.getCachedValue(s0, s1, Clear)`. If a valid cached value
  exists, it is used as a boolean. Otherwise TLC runs `gen(lv.expr, acts,
  lv.con)` (`:1134-1140`). A cached value is valid when it is the same tool,
  every variable assigned at caching time has the same value now
  (`TLCState.isSubset`, `tlc2/tool/TLCState.java:246-271`), and the control is
  equivalent (`tlc2/value/impl/LazyValue.java:143-153`). The decision therefore
  depends on history: an argument already evaluated as a value, for example
  in an earlier guard, is tested as a boolean and is no longer enumerated.
- **A Java override (`OpValue`) or any other value**, such as a constant or a
  start-up value of a constant definition. TLC evaluates it, and it must be a
  boolean (`getNextStatesApplUsrDefOp`, `:1167-1183`).
- **A function application used as an action, `f[a]`** (`:1263-1289`). TLC
  evaluates `f` with `KeepLazy`. A `FcnLambdaValue` without a materialized
  record has its body enumerated in `getFcnContext`
  (`Tool.java:3997-4074`): the parameters are bound to the argument after a
  domain check and tuple matching. Any other function is applied, and the
  result is tested as a boolean. In `NxFcnAction`, `Act[i]` with
  `x' \in {x + i, x + i + 10} /\ x < 20` generates 4 successors for each of the
  20 states with `x < 20`, plus 1 initial state: 81 generated, 32 distinct.

dive:

- `Gen::user_op` (:1331) with `Core::op_target` (:1378), `def_target` (:1424)
  and `opval_target` (:1467). A precomputed constant definition is a value.
  Otherwise it is a body with lazily bound arguments.
- The cache check is `Thunk::cached` (`crates/dive-eval/src/env.rs` :565),
  using `st_cache`.
- `Gen::fcn_apply` (:1277) with `Core::fcn_context` (:1714). Pending `EXCEPT`s
  and arguments TLC would reject are refused.

### 4.10 Anything else: a boolean guard

Every other built-in is evaluated as a boolean in `(s0, s1, Clear)`
(`:1538-1550`). That covers `~`, comparisons, `x' # e`, membership with a
non-variable left side, `ENABLED`, unbounded quantifiers and the rest. If the
result is TRUE, `cont(acts)` runs. If it is FALSE, nothing is generated. A
non-boolean result is `TLC_EXPECTED_EXPRESSION_IN_COMPUTING`. Reading an
unassigned primed variable is an evaluation error. dive: `Gen::default_rule`
(:835).

`\cdot` (`:1488-1532`) counts its intermediate states as generated
(`nss.incrementStatesGenerated(sz)`, `:1507`) when TLC's experimental switch
is on. dive's handling and its divergence are D-022
(`crates/dive-eval/src/next/cdot.rs`, `crates/dive-eval/tests/cdot.rs`).

### 4.11 When a successor counts as generated

| Situation | Generated | Then |
|---|---|---|
| Continuation empty, every variable assigned | yes | constraints, seen set, invariants, implied actions (section 8.4) |
| The same state emitted again, from the same or another action | yes, each time | seen, so no invariant check; implied actions still checked (`ImpliedActionSeen`) |
| Emitted, outside `CONSTRAINT` or `ACTION_CONSTRAINT` | yes | not inserted; invariants still checked (`ConstrainedViolation`) |
| Continuation empty, a variable unassigned | yes | `TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT` (2109), with the action name when there are several actions (`ModelChecker.doNextSetErr`, `ModelChecker.java:588-611`); `NxUnassigned`: 2 generated, 1 distinct |
| A guard false, an equality or membership fails, an `UNCHANGED` variable changed | no | — |
| Evaluation error part-way through | everything before it | the error (`EvalErrorMidLevel`) |

## 5. Fast paths and warnings

The successor shortcut (section 4.0, `fast`) runs only when `TLCGlobals.warn`
is on (`Tool.java:1006`). `-nowarning` turns it off. The initial-state shortcut
always runs (`Tool.java:537`). So the warning switch changes the number of
successors generated, and it is part of the semantics.

`NxFastPath` shows the difference
(`crates/dive-eval/tests/fixtures/next/specs/NxFastPath/`):

```tla
Next == /\ x < 3
        /\ x' = x + 1
        /\ (y' = 1 \/ y' = 1)
        /\ (TRUE \/ TRUE)
        /\ \E i \in {1, 2, 3} : TRUE
```

`x' = x + 1` assigns `x`, and each disjunct of `y' = 1 \/ y' = 1` assigns `y`.
That gives 2 branches.

- **With warnings on.** Both branches have every variable assigned, so the last
  two conjuncts are evaluated as booleans. Each of the 3 states with `x < 3`
  generates 2 successors: 1 + 3 × 2 = **7** generated, 4 distinct
  (`tlc-2026.10.06.014338/NxFastPath.stdout`).
- **With `-nowarning`.** The same conjuncts are enumerated: 2 × 2 × 3 = 12 per
  state, so 1 + 3 × 12 = **37** generated, 4 distinct
  (`NxFastPath.nowarning.stdout`; the command is in `NxFastPath.nowarning.command`).
- **`NxInitFastPath`.** It generates 4 states in both modes, because initial
  states always take the shortcut.
- **`NxUnchangedChanged`.** Its counts are 3 in both modes. With `-nowarning`,
  no warning is printed.

dive: `Options::warnings` (`crates/dive-eval/src/options.rs` :99-104) is `true`
in both profiles, as in TLC. `crates/dive-eval/tests/next_probes.rs`
`no_warnings_matches_tlc` (:251) checks the three `.nowarning` transcripts. The
difference is documented in `docs/tlc-differences.md` and listed with the
`NxFastPath` evidence in `docs/divergences.md`.

For the new core, the switch is a constant at plan-compile time. The compiler
selects one continuation strategy per run and keeps no runtime flag in the
inner loop.

## 6. Lazy-set enumeration order and multiplicity

`\E`, `\A`, `CHOOSE`, filters and maps enumerate in normalized order.
`x' \in S` (and `x \in S` in `Init`) calls `elements()`, which has no
normalization (`Tool.java:1455`, `:836`). For a lazily represented set, the
order and multiplicity depend on the set's representation
(`crates/dive-eval/src/tlc_enum.rs`, module documentation):

| TLC set class | `elements()` |
|---|---|
| `SetEnumValue`, `IntervalValue` | normalized |
| `SubsetValue` | normalized (`elementsNormalized`) |
| `SetOfFcnsValue`, `SetOfRcdsValue`, `SetOfTuplesValue` | odometer over the components, **last** position fastest |
| `SetCupValue` | `S`'s elements, then `T`'s, duplicates included |
| `SetCapValue`, `SetDiffValue` | the enumerable operand's elements, filtered |
| `UnionValue` | each member set in turn, duplicates included |
| `SetPredValue` | the bound set's elements, filtered |

`S \cup T` stays lazy only when neither side is `Reducible`, which in practice
means neither is a set enumeration (`OPCODE_cup`, `Tool.java:2708-2719`).
`SetCupValue.elements()` walks `set1` and then `set2` (`SetCupValue.java:265-310`).

`NxInLazyCup`, `r' \in [a : {1}] \cup [a : {1, 2}]`, therefore emits
`[a |-> 1]` twice from every state. It generates 1 + 3 × 3 = **10** states,
3 distinct.

The rule depends on history. Once TLC compares, sizes or fingerprints the same
lazy object, `convertAndCache` stores a normalized copy, and from then on
`elements()` walks that copy (`SetCupValue.java:265-270`). dive does not track
object identity. When a lazy set's raw sequence differs from its normalized
one, in order or in multiplicity, dive refuses (`Core::tlc_elements`,
`tlc_enum.rs` :269-300). When the two coincide, which is the usual case, the
history does not matter. `rule_fixtures_match_tlc` expects that refusal for
`NxInLazyCup` (`next_probes.rs` :239-243). Two fixtures pin the order of the
cases dive accepts:

- `NxInRcdOrder`: records, last field fastest;
- `NxInLazyFilter`: a filter over `[D -> R]` in the function set's order.

In the default profile (D-010), the order is the run's value order and the
multiplicity is still TLC's (`tlc_enum.rs` :283-286).

The new core keeps the representation class beside the value (the "lazy view"
in `README.md`'s glossary). It must reproduce this table and keep the refusal.
Reproducing `convertAndCache` would mean emulating TLC's object identity, which
ADR 0086 rules out. A proposal to do better belongs in
`docs/open-questions.md`.

## 7. Laziness inside the generator

The generator binds names in four ways. Each way changes what can be
enumerated and how often:

| Binding | TLC | Cached? | dive |
|---|---|---|---|
| Operator argument | `getOpContext(..., cachable = true)`: a `LazyValue` per argument | yes, history-sensitive (section 4.9) | `callee_env`, cachable `Thunk` |
| Zero-arity `LET` definition | not bound; the name's lookup finds the `OpDefNode` | no: re-enumerated at every use | `bind_let_uncached` (:1692) |
| Instance substitution | `getVal(..., false)`: an uncached `LazyValue` | no | `bind_subst(..., false)` |
| `\E` and `\A` variables, function parameters, inlined split parameters | values | n/a | `Sv::Val` |

`getVar` and `getPrimedVar` see through every lazy binding to the variable
beneath it. That is what lets an argument, `LET` value or substitution be the
left side of an assignment. dive: `Core::var_lookup` (:1600).

Fixture counts for these rules (TLC, generated/distinct):

- `NxLetAction`: 5/3;
- `NxLazyArgVar`: 5/4;
- `NxFcnAction`: 81/32;
- `NxInstance`: 4/4.

No rule fixture yet pins the cached-argument case, where an argument first
evaluated as a value is later found cached by the generator. Section 9 lists
it as a fixture to add.

### Conformance checklist: rule fixtures

From `crates/dive-eval/tests/fixtures/next/README.md`. The TLC column gives
generated/distinct. Every row must pass on the new core.

| Fixture | TLC | What it pins | Section |
|---|---|---|---|
| `NxForallOrder` | 3 / 3, `Inv` violated | `\A` in an action: the first binding, then the others last first | 4.3 |
| `NxFastPath` | 7 / 4 (37 / 4 with `-nowarning`) | once every variable is assigned, the rest of the action is evaluated, not enumerated | 5 |
| `NxUnchangedChanged` | 3 / 3 | `UNCHANGED` of a variable already assigned another value generates nothing (warning 2143) | 4.6 |
| `NxStepAngle` | 10 / 4 | `<A>_v` drops unchanged successors; `[A]_v` adds the stuttering one | 4.7 |
| `NxInLazyCup` | 10 / 3 | `x' \in S \cup T` of lazy sets lists duplicates; dive refuses it | 6 |
| `NxInLazyFilter` | 3 / 3, `Inv` violated | `x' \in {f \in [D -> R] : P}` in `[D -> R]`'s order | 6 |
| `NxInRcdOrder` | 3 / 3, `Inv` violated | `x' \in [a : S, b : T]`, the last field fastest | 6 |
| `NxLazyArgVar` | 5 / 4 | an argument naming a variable assigns it, in `Init` and `Next` | 4.9, 7 |
| `NxIfCase` | 8 / 7 | `IF` and `CASE` choose the branch to enumerate | 4.4 |
| `NxLetAction` | 5 / 3 | a `LET` action used twice is enumerated twice | 4.8, 7 |
| `NxEnabled` | 4 / 4, `Inv` violated | `ENABLED` through TLC's `enabled` search | 1 (out of scope here) |
| `NxFcnAction` | 81 / 32 | a lazy function applied as an action (`getFcnContext`) | 4.9 |
| `NxSubseteq` | 17 / 4 | `x' \subseteq S` enumerates `SUBSET S` | 4.5 |
| `NxUnassigned` | 2 / 1, error 2109 | a successor with an unassigned variable is generated, then reported | 4.11 |
| `NxInitFastPath` | 4 / 2 (also with `-nowarning`) | the same shortcut for initial states | 3, 5 |
| `NxInitDup` | 5 / 2 | duplicate initial states are generated | 3 |
| `NxInstance` | 4 / 4 | an instance action assigns the variable its variable is substituted by | 4.8 |
| `NxForallEmpty` | 3 / 3 | `\A` over the empty set is its continuation | 4.3 |

## 8. Design for the new core

### 8.1 Compile the rules into the plan

Each action becomes a generator program built from the plan (`04-plan-compiler.md`).
Its node kinds correspond one to one to section 4:

- `Assign{var, op, rhs}`
- `Conj` (pushes continuation frames)
- `Disj`
- `Exists{bounds}` and `Forall{bounds}`
- `If`, `Case` and `Implies`
- `Unchanged{Var | Tuple | Follow | Verify}`
- `StepBox` and `StepAngle`
- `Inline{body, lazy args}`
- `ThunkAction`, which checks the cache at run time
- `FcnAction`
- `Guard`

The compiler resolves `getPrimedVar` and `getVar` statically wherever the chain
of definitions, `LET` and substitutions is static. It keeps a run-time
resolution node only where the target is a formal parameter's thunk.

The continuation list becomes a per-worker stack of frames, each frame holding
a kind, a node and an environment. The stack must stay persistent in effect,
because `\A` and `[A]_v` run two continuations from the same tail. Frames
therefore come from the worker's arena, and they are released when the state's
expansion ends. A counter of assigned variables makes `all_assigned` O(1). When
`warn` is on, each continuation frame also has a compiled boolean form, which
the shortcut uses.

**Permitted.** An optimization is allowed only if the emitted sequence stays
identical, which includes duplicates. For example, the core may hoist
constant-level domains and sets, keep normalized constant sets across states,
and bind without allocating.

**Forbidden.**

- Merging duplicate disjuncts or bindings.
- Reordering conjuncts. Their order decides which variable is assigned first,
  and therefore where the shortcut begins.
- Skipping an enumeration whose results are known to be duplicates.
- Evaluating a guard earlier than TLC does. An early guard can raise errors
  that TLC never reaches, or reach them in a different order.

### 8.2 A reusable successor buffer with a delta over the parent

Each worker owns one buffer per expansion. The buffer holds:

- a reference to the parent's components;
- `slot[nvars]` of value handles;
- an `assigned` bit set and counter;
- a `changed` list of the variables whose handle differs from the parent's.

`bind` and `unbind` keep all of these up to date, and `unbind` restores the
previous state exactly. This is TLC's single-buffer discipline (section 4.0).

`emit` passes a *view* to the sink: the parent, the buffer and the action and
ordinal. The view is valid until the generator resumes. Nothing is copied at
emission. Values created while generating, such as a new set or record, live in
the worker's arena (`03-value-model.md`). A successor with an incomplete
`assigned` set is still emitted, and the sink reports it. No hash is computed
for it.

### 8.3 Incremental state hash from cached component hashes

The probe key must be bit-identical to the key the reference store computes for
the materialized state. Otherwise distinct counts can differ through
collisions, and fingerprint-set statistics diverge.
`dive_store::state::StateFingerprinter` (`crates/dive-store/src/state.rs`) has
two schemes, and each can be made incremental without changing its value:

- **dive's scheme.** This is the default profile. Each variable's value is
  fingerprinted with FP64, and the value fingerprints are combined by a
  sequential keyed chain, `h := mix64(h ^ mix64(fp_i ^ key(i)))` (`Chain`,
  `combine`). The successor reuses the parent's value fingerprint for every
  variable not in `changed` and computes the others, with per-value caching.
  It then reruns the chain from the first changed index. To do that it starts
  from the parent's chain state at that index, or recomputes the chain, which
  costs two mixer calls per variable.
- **TLC's scheme.** This is `--compat=tlc`: one FP64 stream over the values in
  TLC's variable order. It cannot reuse value fingerprints. It can reuse the
  parent's stream state up to the first changed variable in TLC's order and
  stream from there.
- **`VIEW` and `SYMMETRY`.** The key is computed over the view value or the
  canonical permutation (`TLCStateMut.fingerPrint`). It is computed on the
  buffer view, in the arena, with no incremental shortcut.
- **Exact store.** In the exact store (`state_key`, canonical encoding), the
  buffer view is encoded into an arena scratch buffer for the probe.

### 8.4 Dedupe before materialize

This is "intern after claim" in `README.md`'s glossary. For each emitted view,
the sink follows `Worker.addElement` (`Worker.java:418-480`):

```text
on_emit(s0, action, ordinal, view):
  generated += 1                                   -- first, unconditionally
  if not view.complete(): report 2109; stop        -- already counted
  in_model := state_constraints(view) and action_constraints(s0, view)
  unseen := true
  if in_model: unseen := seen.claim(key(view))     -- probe and insert in one step
  if unseen: check invariants on view              -- out-of-model states included
  check implied actions on (s0, view)              -- seen states included
  if in_model and unseen:
    materialize view into the persistent store; log (parent, ordinal); enqueue
```

Only winners are interned. A violating state that must appear in a trace is
rebuilt by replay, or materialized from the still-valid view, before the arena
is reset. The arena is reset only after every emitted view of the expansion has
been fully checked.

This must not change any observable count:

| Observable | Why dedupe before materialize preserves it |
|---|---|
| States generated | Counted at emission, before the probe. It counts every attempted successor: duplicates, seen states, out-of-model states and incomplete states. Whether a successor is materialized has no effect on the count |
| Distinct states | The claim inserts exactly the in-model states not seen before, as `isSeenState` does. The violating state is inserted before its invariants are checked, so it is included (`MidLevelViolation`). The key equals the reference key (8.3), so membership decisions are identical, including collisions |
| Left on queue, depth | Only in-model unseen states that pass their checks are enqueued, as in TLC (`MidLevelViolation`, `ConstrainedViolation`) |
| Deadlock | Decided by generated (zero emissions), not by winners (`ConstraintNoDeadlock`) |
| Counts at a stop, which violation, error ordinal | The probe runs after emission and does not reorder anything, so the emission sequence is unchanged |
| Traces | Winners log `(parent, ordinal)` exactly as now. Replay regenerates by ordinal against the same sequence |

The scheme is safe because every decision after emission depends only on the
variables' values. Those are the same whether they are read from the parent
plus the delta, or from a materialized copy. Three conditions make this hold:

1. The evaluator reads state variables through one interface that is
   implemented by both the view and the stored state.
2. The key is a function of values, not of representation. Lazy and normalized
   forms of one set must hash alike (`03-value-model.md`).
3. A `LazyValue`-style thunk cache records copies of the states it was computed
   in (`LazyValue.getValue`, `LazyValue.java:169-182`), never a reference to the
   mutable buffer.

Parallel claims and determinism at N workers are covered in `07-engine.md`.

## 9. Test obligations

The new core must pass all of the following before it replaces the reference
engine on any path:

- **Rule fixtures.** These are the 18 fixtures in section 7: 17 must agree with
  TLC and `NxInLazyCup` must be refused. The same harness as
  `crates/dive-eval/tests/next_probes.rs` (`rule_fixtures_match_tlc`) checks
  verdict, generated, distinct, queue, depth, trace length and warnings. The
  three `-nowarning` transcripts must pass with warnings off
  (`no_warnings_matches_tlc`).
- **TLC probes.** The 19 probes in `crates/dive-check/tests/fixtures/tlc-probes/`
  (`probes_match_tlc`). These cover counts at a stop, constraints, deadlock,
  stuttering, duplicates, `\E` order, set order and implied actions on seen
  states.
- **Action splitting.** `crates/dive-ir/tests/actions.rs` against TLC's action
  dump (fixtures `Split*`). Labels in traces must be identical.
- **Corpus.**
  - `conformance/gates/smoke.txt` at 1 and 4 workers.
  - `cargo xtask gate --full`, which runs `conformance/gates/cm1.txt` and the
    validation-smoke corpus at 1, 4 and 8 workers, then the small corpus tier in
    both profiles, against `conformance/baselines/`.
  - Both profiles must be run, because the order rules interact with D-010
    and D-022.
- **Shadow comparison of successors** (`08-correctness.md`). For every state
  the new core expands in a shadow run, the reference engine runs
  `Evaluator::successors` (`next.rs` :522) on the same state, and
  `Evaluator::init_states` (:439) is compared once. Successors are compared as
  `(action id, values, complete)`:
  1. **Multisets of successors.** Equal multiplicity proves equal generated
     counts, and a mismatch is reported first, as the clearest diagnostic.
  2. **Sequences.** Equal sequences, with ordinals, prove equal counts at a
     stop and replayable traces.
  3. **Side outputs.** The per-action emission counts, the warnings printed
     and the first error, with the number of successors emitted before it,
     must also match.

  Any difference is a bug in the new core, never a verdict.
- **Fixtures to add** before the corresponding optimization lands. Each is the
  smallest spec that shows the case, with the oracle's transcript and generated
  by `dive-conformance dev capture`:
  - a cached operator argument evaluated as a guard and then used as an action
    (section 4.9);
  - `[A]_v` where `A` leaves `v` unchanged, which generates the step twice;
  - a state whose only successors are already seen, which is not deadlocked;
  - duplicate successors of one parent, of which only the first is claimed and
    logged.

## 10. Next step

M0 implements the rules of sections 3 and 4 that the spike models use:
- conjunction lists with continuations, `\/`, `\E` and `\A` (in TLC's
  binding order), and `IF`, `CASE` and `=>`;
- `x' = e` and `x' \in S`, with a filter streamed from its bound set;
- `UNCHANGED` of variable tuples, and `[A]_v`;
- the all-assigned shortcut, and initial-state enumeration over the same
  generator.

It refuses `x' \in` a union, where TLC's multiplicity is not reproduced, and
`x' \subseteq S`. Its generated counts equal TLC's on all four spike models
and on the fixtures (`m0-results.md`). Enumeration uses canonical order, not
TLC's order. Counts do not depend on it at one worker, but traces and runtime
names will.

Next (M1, stream 3):
1. Copy the rule list of sections 3 and 4 into `02-semantic-contract.md` as
   contract items, each citing its fixture.
2. Write the four fixtures listed at the end of section 9 and capture their TLC
   transcripts.
3. Complete the generator node set of section 8.1: the remaining set
   constructors under `\in`, warnings 2143 and friends, TLC enumeration order
   under compat, and the generator side of `ENABLED`.
