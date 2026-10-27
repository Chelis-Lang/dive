# Semantic contract

Status: design contract, 2026-10-26. There is no new-core code yet. Every
citation below was read at the time of writing. When a rule here and the
reference engine disagree, the pinned TLC golden decides, and this document is
corrected with the evidence.

## 1. Purpose and scope

This document lists what the new core (`dive-core`, `dive-explore`) must
reproduce and what it may drop. "Reproduce" means the fields of section 2 come
out the same. It does not mean the same internal steps (ADR 0086). Anything
the new core cannot reproduce exactly is refused with an explicit
`unsupported` result that names the feature, as the reference engine does
today (`AGENTS.md`, "Soundness first").

**Profiles.** Each rule names the profile it applies to:

- **tlc**: `--compat=tlc`. The rule must match pinned TLC.
- **default**: dive's default profile. It equals TLC except for the registered
  divergences in section 5.
- **both**: both profiles.

The two option sets are `Options::tlc()` and `Options::dive()`
(`crates/dive-eval/src/options.rs:121-150`):

| Setting | tlc | default |
|---|---|---|
| Value order | TLC intern-token order | canonical order |
| Exploration order | TLC order | the run's value order (D-010) |
| Comparisons | TLC | TLA+ (D-009) |
| Integers | 32-bit | big integers (D-004) |
| Action composition | off | TLA+ meaning (D-022) |

All-assigned warnings are on in both.

**TLC sources.** Paths of the form `tlc2/...` and `util/...` are relative to
`tlatools/org.lamport.tlatools/src/` in the pinned tlaplus checkout (commit
`94d0c50`), which `dive-conformance corpus fetch` places in the harness cache.

## 2. The observable fields

The harness compares a run with its golden on these fields only
(`crates/dive-conformance/src/compare.rs:1-26`, `diff_results_with` at
`:175-258`). Queue population is recorded but not compared (`:14`). The
contents of a counterexample are not compared; instead every counterexample
must validate in `dive-trace` (`:17-19`; ADR 0016).

| Field | Rule | Citation |
|---|---|---|
| `verdict` | always compared | `compare.rs:183` |
| `error_class` | the `EC` name of the *deciding* message | `compare.rs:5-6`, `:184-189` |
| `exit_code` | the numeric process exit, never waived | `compare.rs:190-191` |
| `violated_property` | compared when the golden names one | `compare.rs:192-200` |
| `counts.distinct_states` | compared at every worker count | `compare.rs:205-211` |
| `counts.states_generated` | compared at 1 worker for native runs | `compare.rs:212-221` |
| `counts.depth` | always compared for native runs | `compare.rs:223-229` |
| `trace.length`, `trace.ending` | compared unless both runs are periodic liveness words | `compare.rs:230-257` |

Native implementations get no multi-worker leniency (`compare.rs:21-33`). D-001
further requires generated counts to be identical at every worker count.

**Exit codes.** `exit_statuses_for` (`crates/dive-conformance/src/tlc/codes.rs:384-399`)
mirrors `EC.ExitStatus.errorConstantToExitStatus`. The cases the core produces:

| Deciding code | Exit |
|---|---|
| 2103 (`TLC_NESTED_EXPRESSION`), 2109, 2115, 2147 (`TLC_FINGERPRINT_EXCEPTION`) | 75 |
| 2111 (`TLC_INVARIANT_EVALUATION_FAILED`), 2146 | 76 |
| 2107, 2110 | 12 |
| 2108, 2112, 2113, 2116 | 13 |
| 2114 (deadlock) | 11 |
| 2104, 2105 (assumptions) | 10 |
| 2132 (`Assert`) | 14 |
| any code outside the switch, for example 1000 `GENERAL`, 2178 overflow, 2179 division by zero | 255 |

The verdict class for each code is `verdict_for_code` (`codes.rs:413-440`).
Under D-023, the default profile exits with TLC's statuses;
`--exit-codes native` restores dive's earlier statuses.

## 3. Rules the new core MUST reproduce

### (a) Successor generation order and counting

**A1. Generated is the number of successors the enumerator emits, duplicates
included.** The generator never deduplicates, filters or reorders.
- Profile: both.
- Evidence: `tlc2/tool/Worker.java:418-420` (`statesGenerated++` first);
  `crates/dive-eval/src/next.rs:6-8`, `:38-41`; probe `DuplicateSuccessors`
  (`crates/dive-check/tests/fixtures/tlc-probes/specs/`).
- Core: counting is one increment per emitted successor. Deduplication before
  materialization must not skip the increment.

**A2. Conjunctions run left to right as a continuation; disjunctions run each
disjunct left to right with the same continuation.**
- Profile: both.
- Evidence: `tlc2/tool/impl/Tool.java:1189-1219`; `next.rs:862-882`.
- Core: the successor order is the depth-first order of this continuation
  tree. Short-circuiting is allowed only where it emits the same sequence.

**A3. `\E x \in S, y \in T : A` enumerates bindings in normalized order, with
the first variable varying fastest.**
- Profile: both. Under tlc the order is TLC's value order; under default it is
  the run's value order (D-010).
- Evidence: `tlc2/tool/impl/Tool.java:1220-1244`, `:4083-4086`
  (`Ordering.NORMALIZED`); `tlc2/tool/impl/ContextEnumerator.java:62-70`;
  `next.rs:884-910`; probes `ExistsOrder` and `ExistsSetOrder`.

**A4. `\A x \in S : A` enumerates the first binding's `A`, and pushes the other
bindings onto the continuation. They therefore run last first.**
- Profile: both.
- Evidence: `Tool.java:1245-1262`; `next.rs:912-930`; fixture `NxForallOrder`
  (3 generated / 3 distinct, `Inv` violated). `NxForallEmpty` shows that
  `\A` over the empty set is just its continuation.
- Core: with `S = {1, 2, 3}` the visit order is 1, 3, 2.

**A5. `x' = e` always evaluates `e` first.** If `x'` is unassigned it binds
`x'`; otherwise it compares with TLC's `equals`, which can raise an error.
- Profile: both. The comparison follows the profile's comparison setting.
- Evidence: `Tool.java:1353-1378`; `next.rs:1043`.
- Core: `e` is evaluated even when `x'` is already assigned, so its errors
  happen.

**A6. `x' \in S` binds each element in TLC's `elements()` order, not
normalized order.** This applies to sets with a lazy representation. Products,
record sets and function sets run as odometers with the last position fastest.
A union lists its left operand, then its right, duplicates included. A filter
uses its bound set's order. Where `elements()` depends on whether TLC has
already normalized the same set object, the reference engine refuses.
- Profile: both. Under default the order is the run's value order, but the
  multiplicity is still TLC's.
- Evidence: `Tool.java:1422-1468`; `crates/dive-eval/src/tlc_enum.rs:1-28`,
  `:269-298` (refusal), `:461-473`, `:476`; fixtures `NxInRcdOrder`,
  `NxInLazyFilter`, and `NxInLazyCup` (TLC 10 generated / 3 distinct; dive
  refuses); `tlc2/value/impl/SetCupValue.java:215` (`convertAndCache`), `:265`.
- Core: keep the representation class of `S` until enumeration (lazy view,
  `03-value-model.md` §8). Keep the refusal when the order depends on history.

**A7. `x' \subseteq S` enumerates `SUBSET S`.**
- Profile: both.
- Evidence: fixture `NxSubseteq` (17 / 4).

**A8. `UNCHANGED e` is decomposed by `processUnchanged`.** A variable is bound
or compared. A tuple pushes its items. A zero-arity definition is followed.
`UNCHANGED x` with `x'` already assigned a different value generates nothing,
with warning 2143.
- Profile: both.
- Evidence: `Tool.java:1484-1487`, `:1557`; `next.rs:22`, `:1210`; fixture
  `NxUnchangedChanged` (3 / 3).

**A9. `[A]_v` enumerates `A`, then the `UNCHANGED v` successor. `<A>_v`
pushes a `CHANGED v` check onto the continuation.**
- Profile: both.
- Evidence: `Tool.java:1290-1305`; `next.rs:1003-1018`; fixture `NxStepAngle`
  (10 / 4); probes `StutterInNext`, `StutterNext`.
- Core: the stuttering successor of `[A]_v` is counted as generated even when
  it duplicates one of `A`'s successors.

**A10. All-assigned fast path.** Once every variable is assigned and warnings
are on, TLC evaluates the rest of the continuation as booleans: one successor,
not one per disjunct or binding. In `Init` the path is unconditional.
- Profile: both. Warnings are on by default (`tlc2/TLCGlobals.java:217`). The
  harness never passes `-nowarning`.
- Evidence: `Tool.java:1006-1016`, `:1047`; `next.rs:733-739`, `:788`;
  fixture `NxFastPath` (7 generated, but 37 with `-nowarning`);
  `NxInitFastPath` (4 / 2).
- Core: this is a counting rule, not an optimization. Generated depends on it.

**A11. Other enumeration rules.**
- `IF` and `CASE` evaluate the guard, then enumerate the chosen branch
  (`NxIfCase` 8 / 7).
- A lazy function applied as an action is enumerated through its body
  (`Tool.java:1263-1289`; `NxFcnAction` 81 / 32).
- An operator argument that names a variable assigns it (`NxLazyArgVar`).
- An instance action assigns the variable its own variable is substituted by
  (`NxInstance`).
- Duplicate initial states are generated and counted (`NxInitDup` 5 / 2).
- A successor with an unassigned variable is counted, then reported as 2109
  (`NxUnassigned` 2 / 1; `Worker.java:423-429`).

All fixtures are under `crates/dive-eval/tests/fixtures/next/specs/`, with
TLC transcripts in `tlc-2026.10.06.014338/`.

**A12. Per-successor check order and the stop.**
1. Count the successor.
2. Check that every variable is assigned.
3. Evaluate `CONSTRAINT` and `ACTION_CONSTRAINT`.
4. If the successor is in the model, insert it.
5. Check invariants only on new states or states outside the model.
6. Check implied actions on every pair.
7. Enqueue new in-model states.

Deadlock means no successor was generated. Successors outside the model count.
At a stop, the counts and depth follow ADR 0015 §3.
- Profile: both.
- Evidence: `Worker.java:418-476`; ADR 0015 "TLC's single-worker search,
  exactly" and §1–§3; probes `ConstrainedViolation`, `ImpliedActionSeen`,
  `DeadlockMidLevel`, `EvalErrorMidLevel`, `MidLevelViolation`.

### (b) Laziness

**B1. Laziness decides whether an error happens.** TLC creates a `LazyValue`
for:
- user-operator arguments;
- zero-arity `LET` definitions (in evaluation);
- `INSTANCE` substitutions.

Nothing else is lazy. An argument that is never forced never raises its error.
- Profile: both.
- Evidence: `tlc2/tool/impl/SymbolNodeValueLookupProvider.java:123-131`;
  `Tool.java:1849-1859`; ADR 0014 §2.
- Core: evaluate a lazy site eagerly only under the demand-order conditions
  of `04-plan-compiler.md` §4. The site must be demanded on every path, and no
  failing or effectful expression may come between the binding and the first
  use.

**B2. LazyValue cache reuse follows `getCachedValue` exactly, including its
stale reuses.** A forced value is reused when three conditions hold:
- the current states include the cached states, comparing only root-module
  variable names (`TLCState.isSubset`);
- the evaluation control flags are equivalent;
- the successor state is not null.

When the successor state is null, TLC neither reads nor fills the cache.
Because `isSubset` ignores hidden `INSTANCE` variables, TLC reuses a value
that a fresh evaluation would change.
- Profile: tlc only. The default profile evaluates soundly (§3a below).
- Evidence:
  - `tlc2/value/impl/LazyValue.java:143-153`, `:169-183`;
    `tlc2/tool/TLCState.java:246-271`; `Tool.java:1922-1940`;
  - `crates/dive-eval/src/env.rs:67-90`, `:509-597`;
    `crates/dive-eval/src/eval.rs:703-735`;
  - corpus `conformance/corpus/lazy-enabled-cache/` (`selection.json`).
    `hidden_remove_two` and `hidden_unchanged` give 2107 with exit 12: the
    stale `Cut` makes `ENABLED` false. `hidden_remove_one`,
    `hidden_singleton_two`, `hidden_inline_remove_two` and `root_remove_two`
    succeed with 1 distinct, 2 generated, depth 1;
  - `docs/user/temporal.md`, "Hidden-query lazy argument caching". A
    diagnostic run with `-Dtlc2.value.impl.LazyValue.off=true` flips only the
    remove-two control.
- Core, `--compat=tlc`: wherever such a site is reachable (under `ENABLED`,
  behind an `INSTANCE` with hidden variables), keep a real thunk with TLC's key.
  A cache that is semantically sound would give TLC's answer wrongly here.
- Core, default profile: reuse a value only when every input it depends on is
  unchanged, hidden `INSTANCE` variables included. The stale reuse is not
  reproduced. This changes the default-profile result of the affected corpus
  controls; see §3a.

**B3. `ENABLED` cutoff.** `ENABLED` evaluates its body in
`Context.branch(c)`. A primed lookup stops at the branch, so substitutions made
outside the `ENABLED` are hidden from it.
- Profile: both.
- Evidence: `Tool.java:2581`; `tlc2/util/Context.java:57`, `:123-135`;
  `env.rs:403-450`; `eval.rs:768-781`; corpus
  `conformance/corpus/lazy-enabled-capture/`.
- Core: inlining substitutions by name (`04-plan-compiler.md`, pass 2) is not
  allowed under `ENABLED` or a prime cutoff.
- Open: this cutoff may be TLC implementing correctly the rule that
  substitution does not distribute over `ENABLED`. It stays in both profiles
  until it is ruled on (§6).

**B4. Lexical lifetime of lazy bindings in queries.** A `LET` reached before
a hidden membership choice keeps one binding across all the candidates. A
`LET` reached after the choice gets a new binding for each candidate.
- Profile: tlc only. The default profile evaluates soundly (§3a below).
- Evidence: corpus `conformance/corpus/lazy-query-lifetimes/` (24 controls).
  For example, `delayed_let_remove_two` gives 2107 with exit 12, while
  `delayed_let_remove_one` succeeds. The paired `*_inverse` invariants catch
  fabricated endings. `docs/user/temporal.md`, "Lazy query binding lifetime".
- Core, `--compat=tlc`: create a binding when the `LET` or application is
  reached, never per complete candidate and never hoisted.
- Core, default profile: a `LET` value that depends on primed variables is
  evaluated for each candidate, as TLA+'s definition-by-substitution requires.

**B5. The generator does not cache `LET` definitions or substitutions.**
- In `getNextStates`, a `LET` binds nothing. Each use evaluates the
  definition afresh.
- Substitutions are bound uncached.
- A lazy operator argument is used only if it is already cached; otherwise it
  is enumerated as an action.
- Profile: both.
- Evidence: `Tool.java:937-940`, `:964-985`, `:1134-1140`;
  `next.rs:1019-1029`, `:1689-1707`; fixture `NxLetAction` (`A \/ A` with
  `A` a `LET` action: 5 / 3).
- Core: this is observable through enumeration counts (A1) and through
  effects (B6). It is not observable through the value of a pure, infallible
  expression.

**B6. Effects depend on evaluation counts.** The number of times an
expression is evaluated changes:
- random draws: TLC redraws a `LET`-bound `RandomElement` at each use in
  `Init` and actions (`crates/dive-stdlib/tests/fixtures/random/specs/RndLetRedraw`;
  `docs/tlc-differences.md` D-002);
- register writes (`TLCSet`) and live `TLCGet` reads.

Draws come from one stream per expanded state, shared by the next-state
relation and the checks on its successors.
- Core: `RandomElement` draws from the reference engine's streams
  (`03-value-model.md` §6.1). A draw in a check, a startup draw before
  `Init` draws, and an effect-bearing `LET` or argument used in more than
  one conjunct are refused.
- Profile: tlc for bit-exact draws; both for "the same seed gives the same
  run".
- Evidence: `crates/dive-model/src/reach.rs:229-248`;
  `crates/dive-stdlib/src/random/seeding.rs:16-31`; ADR 0014 §7.
- Core: any expression that reaches an effect is evaluated exactly as often,
  and in exactly the order, that TLC evaluates it. Memoization and eager
  evaluation exclude it (`06-memoization.md` §4).

**B7. Errors are never cached.** A thunk or set enumeration that fails is
recomputed in the caller's context.
- Profile: both.
- Evidence: `LazyValue.java:169-183` (the cache is filled only after `eval`
  returns); `eval.rs:726-729`; ADR 0012 §1 ("Only successes are cached").
- Core: this is not observable for a pure body whose cache key covers every
  input, because an equal key re-evaluates to the same error. The new core may
  therefore cache errors in both profiles, under the conditions in
  `06-memoization.md` §4. Effectful bodies are never cached.

### (c) Normalization (`isNorm`) and membership errors

**C1. A set enumeration stays unnormalized until TLC normalizes it.** These
operations normalize: comparison, equality, size, fingerprinting, printing and
enumeration. Hashing does not. Until normalization, membership is a linear
scan with `equals`. After it, membership is a binary search with `compareTo`.
So `<<1>> \in {[qfa |-> 1, qfb |-> 1], [qfa |-> 2, qfb |-> 2]}` is an error
until the set has been fingerprinted as part of a state.
- Profile: tlc for the membership outcome. Under default, D-009 answers this
  comparison.
- Evidence: `tlc2/value/impl/SetEnumValue.java:146-148`, `:348-350`;
  `crates/dive-value/src/value.rs:216-229` (`tlc_norm`); fixtures `RvMemUnnorm`
  and `RvMemNorm` (`crates/dive-value/tests/fixtures/specs/`).
- Core: the normalization flag is part of the lazy view, not of the value's
  identity. Interning or hashing must not set it.

**C2. Normalization errors depend on insertion order.** Normalization is
TLC's binary insertion sort, which raises an error for the first incomparable
pair it happens to compare. `TupleValue.compareTo(record)` compares sizes, but
`RecordValue.compareTo(tuple)` fails at once. So `{[qfa |-> 1, qfb |-> 1], <<1>>}`
is a set and `{<<1>>, [qfa |-> 1, qfb |-> 1]}` is an error.
- Profile: both. Set construction keeps TLC's rules under D-009
  (`crates/dive-eval/src/tla_cmp.rs:15-16`).
- Evidence: `tlc2/value/impl/ValueVec.java:172-197`;
  `crates/dive-value/src/order.rs:947`; ADR 0012 §4; fixtures `AsymSet*`,
  `RvCanonAsym`.
- Core: the slow path runs TLC's algorithm in TLC token order in both
  profiles. The result is then re-sorted in the run's order.

**C3. Membership dispatches on the set's TLC class.** For example,
`IntervalValue.member` returns `FALSE` for an untyped model value but raises an
error for a string. A function set, record set, `SUBSET` or product has its
own `member`.
- Profile: tlc. Default answers compound-kind cases by D-009 but keeps TLC's
  error between scalars of different kinds (`tla_cmp.rs:8-12`).
- Evidence: `tlc2/value/impl/IntervalValue.java:75-87`; fixtures `ErrMem*`
  (for example `ErrMemIntvStr`, `ErrMemIntvTyped`, `ErrMemSubsetInt`,
  `ErrMemNormalized`).

**C4. A state is fingerprinted before its invariants only if it is in the
model.** TLC computes a state's fingerprint (and so normalizes its sets) inside
`isSeenState`, which runs only for in-model successors. Invariants on a
successor outside the model therefore see its sets as constructed.
- Profile: tlc.
- Evidence: `Worker.java:434-460`. There is no probe yet (section 6, Q4).
- Core: the deep close at intern time (`03-value-model.md` §3.3) is
  unobservable for in-model states. For states outside the model it is not
  established.

**C5. Record literals and string-domain functions differ.** A record literal
(`RecordValue`) refuses to compare with a tuple. The same function built with
`:>` or `@@` (`FcnRcdValue`) compares sizes first.
- Profile: tlc.
- Evidence: ADR 0012 §1; fixtures `RvFcnRcd1`–`RvFcnRcd8`.

### (d) Value order

**D1. Names compare by intern token, and the rest of the order is built on
that.** Tokens follow encounter order:
1. the configuration;
2. 236 SANY built-ins;
3. lexer tokens in parse order;
4. semantic-analysis names and string literals;
5. runtime names.

Sets compare by size, then by element. Functions compare by domain size, then
domain, then values. Untyped model values are lowest. `FALSE < TRUE`.
- Profile: tlc (`--choose-order=tlc`, `--explore-order=tlc`).
- Evidence: `util/UniqueString.java:227-231`; ADR 0012 "TLC's order" and "For
  the front end"; `crates/dive-model/src/intern.rs`; fixtures `ChooseIntern`,
  `ChooseInternUnused`; probes `StringOrderViolation`,
  `StringOrderPreinterned`.

**D2. The default profile uses canonical order.** Names compare by UTF-8
bytes. Across kinds the order is: model values < booleans < integers <
strings < finite sets < functions < infinite sets. `CHOOSE`, exploration and
printing all follow it.
- Profile: default.
- Evidence: ADR 0012 §3; D-006, D-010.

**D3. Normalization errors use TLC tokens in both profiles** (rule C2). The
residual difference is that membership comparisons in a normalized set follow
the run's order (ADR 0012 §4, D-009).

**D4. Runtime names get tokens in single-worker encounter order.**
- Profile: tlc.
- Evidence: ADR 0015, "Runtime name ordering admission"; `reach.rs:239`; ADR
  0114.
- Core: `07-engine.md` §6.

### (e) Error classes

**E1. The deciding code and the reported code can differ.** The harness
compares the deciding class and the exit. TLC reports an error while
exploring, then replays the predecessor with a `CallStackTool`. The replay's
outcome decides the exit status.
- Profile: both.
- Evidence: `tlc2/tool/ModelChecker.java:291-315`;
  `crates/dive-check/src/class.rs:8-43`.

**E2. Errors in `Next`, constraints, `VIEW` and fingerprinting.**
- An `EvalException` keeps its own code. For example, division by zero is
  2179 with exit 255 (`ErNextDivZero`).
- Any other exception (`Assert.fail`, a wrapped Java-override error) is
  reported as `GENERAL` and decided as 2103 with exit 75 (`ErNextTypeErr`).
- Profile: both.
- Evidence: `class.rs:176-184`, `:186-246`; ModelChecker lines as in E1;
  fixtures under `crates/dive-check/tests/fixtures/errors/`.
- Core: errors carry typed provenance: the `EC` code, and whether the error is
  an `EvalException` or an override. Provenance is never parsed from text
  (`class.rs:36-40`).

**E3. An invariant that fails to evaluate on a new state decides 2111, with
exit 76.** TLC's replay finds the state already inserted and does not check it
again. It goes on expanding the predecessor. It inserts and enqueues later new
states, so they appear in the distinct and queue counts but not in generated.
- A second error during the replay re-classifies the run: `ErInvReplayError`
  exits 75.
- A violation, or an incomplete state, ends the replay quietly.
- A state outside the model is checked again by the replay, which then decides
  (`recurring`).
- Profile: both.
- Evidence: `ModelChecker.java:531-533`, `:615`; `class.rs:204-215`,
  `:248-288`; `crates/dive-check/src/engine.rs:428-446`, `:1698-1864`
  (`replay_after`). Fixture `ErInvDivZero`: 4 generated, 5 distinct, 2 on the
  queue, exit 76; the search alone found 4 distinct and 1 queued. See also
  `ErInvReplayProp` (exit 76) and `ErInvReplayError` (4 / 5 / 1, exit 75).
- Core: the replay is part of the contract because it changes `distinct` and
  `error_class`. It runs with memoization off (`07-engine.md` §4).

**E4. An implied action that fails to evaluate is reported as 2113, and the
replay decides.** The replay checks the step again, so the deciding code is
the replayed error's.
- Profile: both.
- Evidence: `class.rs:216-230`; fixture `ErPropDivZero` (exit 255, decided by
  2179).

**E5. An assumption that fails to evaluate is 2105, with exit 10.**
- Profile: both.
- Evidence: `Tool.java:3645-3663`; `class.rs:317-325`; `ErAssumeDivZero`
  (exit 10).

**E6. Initial-state errors.**
- The reported code is `GENERAL`, or `TLC_INITIAL_STATE` (2102) naming the
  state for a non-`EvalException` raised while checking an initial state.
- The replay (`doInit(cTool, true)`) decides 2103, with exit 75.
- At an initial-state violation the adapter reports no counts.
- Profile: both.
- Evidence: `class.rs:290-315`; `ErInitDivZero` (exit 75); ADR 0015 §3.

**E7. `FingerprintException` wrapping.** Errors from the methods of a value
that has a source location are wrapped, and decide 2147 with exit 75. Whether a
value has a source depends on where it was built, which dive does not track.
Such classes are explicitly unverified, and the harness treats them as
`unsupported`.
- Profile: both.
- Evidence: `ModelChecker.java:302-305`; `class.rs:26-31`, `:86-99`; fixtures
  `ErWrap*`; `crates/dive-check/tests/fixtures/errors/README.md`.
- Core: keep the "unverified, never agreement" status unless value source
  provenance is added (section 6, Q3).

**E8. An error ends the enumeration of the state.** Successors emitted before
it keep their counts (probe `EvalErrorMidLevel`: 4 / 3 / 1; ADR 0014 §3).
- Profile: both.

### (f) Fingerprints

**F1. Under tlc, per-state random draws are seeded with TLC's state FP64 XOR
the seed.** That fingerprint uses TLC's variable order and the `-fp`
polynomial.
- Profile: tlc.
- Evidence: `tlc2/value/RandomEnumerableValues.java:136-143`;
  `crates/dive-stdlib/src/random/seeding.rs:81-91`; ADR 0014 §7 (variable
  order); ADR 0012 §6 (`FpFull`: all 64 bits of 28 states).
- Core: compute FP64 of the state whenever a random draw is reachable from
  `Next` or the checks (`03-value-model.md` §6).

**F2. `TLCFP(v)` returns the low 32 bits of FP64 of `v`, after
`deepNormalize`.**
- Profile: both, wherever it is implemented.
- Evidence: `tlc2/module/TLCExt.java:410-416`; `tlc2/util/FP64.java:221-224`;
  fixture `PrintFp`; ADR 0012 §6 ("Low 32 bits").

**F3. `VIEW` with randomness.** TLC seeds from the view's fingerprint. Both
profiles refuse randomness combined with `VIEW` or `SYMMETRY` before binding.
- Profile: both (the refusal).
- Evidence: `docs/tlc-differences.md` D-002, "Randomness with VIEW or
  SYMMETRY (#178)". ADR 0016 has a superseded addendum, "pure VIEW random
  streams".
- Core: keep the refusal, or implement with FP64 of the view value.

**F4. Default-profile draws are keyed by dive's own state fingerprint.**
- Profile: default.
- Evidence: `seeding.rs:9-14`; `crates/dive-stdlib/src/random/mod.rs:39-41`.
- Core: the key must be a fixed function of the state, independent of workers.
  Whether it may change across versions is section 6, Q2.

**F5. Elsewhere, fingerprints affect only set membership, except through a
collision.**
- Evidence: ADR 0015, "Where order comes from".

### (g) Determinism

**G1. Results are identical at every worker count.** Every compared field must
match at every worker count. Successors carry positions in single-worker
order. The first event in that order decides the result, and counts are
computed up to it.
- Profile: both.
- Evidence: D-001; ADR 0015 §1–§3; `crates/dive-check/tests/invariance.rs`.

**G2. Specs that observe TLC's evaluation order get single-worker results.**
This covers registers, live `TLCGet`, I/O, effects inside lazily evaluated
sets, functions or bound cached thunks, randomness, and runtime names under
TLC order. The run reports that it used one worker.
- Profile: both. Runtime names apply under tlc only.
- Evidence: `reach.rs:229-248`; `engine.rs:2247-2279` (`plan_workers`).

**G3. The seed is always reported, and `--seed S` reproduces the run at any
worker count.** Under tlc, draws match TLC bit for bit for a fixed `-seed` and
`-fp`.
- Profile: both.
- Evidence: D-002; `crates/dive-stdlib/tests/java_random.rs`, `dive_mode.rs`.

**G4. `TLCGet("config").worker` is 1 in the default profile.** Under tlc it
is the physical worker count.
- Evidence: D-011.

**G5. `Print` output is released in single-worker order at every worker
count.**
- Profile: both.
- Evidence: ADR 0014 §7; D-001.

## 3a. Where the default profile does better than TLC

**Owner direction, 2026-10-26.** Where TLC returns a result that TLA+
semantics would not, such as a stale cached value, the default profile does the
semantically correct thing. `--compat=tlc` reproduces TLC. This applies
ADR 0010: the improvement is result-changing, and the owner has approved it as
the default.

| Rule | TLC behaviour | Default-profile behaviour | Divergence |
|---|---|---|---|
| B2 | Reuses a cached lazy value when only hidden `INSTANCE` variables changed | Reuse only when all inputs are unchanged | to register as D-027 with the implementing PR |
| B4 | One `LET` binding kept across hidden-choice candidates | Per-candidate evaluation where the value depends on primed variables | same entry as B2, or its own, decided when implemented |

The current reference engine reproduces both behaviours in both profiles. The
affected controls in `conformance/corpus/lazy-enabled-cache/` and
`lazy-query-lifetimes/` therefore change their default-profile result. The
implementing PR adds:
- their expected-divergence entries;
- the D-entry, in `docs/divergences.md` and `docs/tlc-differences.md`;
- tests in both profiles.

Further candidates are added to this table with evidence. A behaviour is a
candidate only if it differs from the TLA+ semantics, not merely from another
implementation choice.

## 4. Behaviours the new core MAY drop

**X1. Message text and Java signatures.** No compared field holds message
text. ADR 0086 excludes Java method projections.
- Keep: the typed distinctions that decide class and exit (E2).
- Keep: the source spans and stable codes that `AGENTS.md` requires.
- See `04-plan-compiler.md` §5.

**X2. `Print` count and order relative to TLC.** `Print` is not a compared
field, and the reference engine already differs. TLC prints the call-stack
replay's output again; dive does not (`engine.rs:1756`).
- Keep: G5, worker invariance.
- Keep: a `Print` may never change another field.
- Keep: an expression that reaches an effect other than `Print` follows B6.
- D-006 covers the order of `CHOOSE` and of printed values, not how many
  times `Print` runs. A divergence in the number of `Print` calls is not
  registered yet and needs its own D-entry (Q1).

**X3. Fingerprint bits as the seen-set key.** Fingerprints affect only
membership (F5). The goldens pin `-fp 0`, so any 64-bit state hash, or exact
storage, gives the same fields.
- Keep: FP64 for F1 and F2.
- The collision-probability text is output only.
- Under `SYMMETRY` with `VIEW`, the orbit's canonical representative feeds
  the view, so it must stay TLC's minimum-permutation rule (ADR 0015 §6) unless
  the view is shown to be symmetric (Q6).

**X4. Caching of pure, infallible thunks.** For a thunk with no effects that
cannot fail, caching and re-evaluating give the same value, wherever TLC's
reuse is sound.
- Not droppable: the stale reuses of B2.
- Not droppable: the lifetimes of B4.
- Not droppable: anything reaching B6.
- See `06-memoization.md` §4.

**X5. `Arc` and frame representation.** The only observable property of
`Arc<Frame>`, `Arc<SubstLink>` and copy-on-write frames is persistence
(`env.rs:9-13`): a capture sees the bindings at its creation. Any
representation that keeps this persistence may replace them. See ADR 0117,
"Context".

**X6. Forced single-worker runs for runtime names.** The contract is D4:
tokens follow single-worker encounter order. The reference engine enforces it
by running on one worker (`reach.rs:262-310`), and ADR 0015 notes that an
ordered parallel allocator is future work. `07-engine.md` §6 is a valid
replacement. Other reasons for sequential mode (G2) remain unless shown
unobservable.

**X7. The cache-domain mechanism.** `Host::expression_cache_domain` and the
`foreign` thunk slot exist so that independent replay never reads another
consumer's cached values (`crates/dive-eval/src/host.rs:224-233`;
`env.rs:518-560`).
- Keep: each replay has its own TLC-rule cache, which reproduces B2 inside the
  replay (ADR 0103, "Context").
- Keep: no value crosses from one consumer to another.
- The mechanism itself may change.

**Also droppable under ADR 0086:**
- queue population;
- lasso and SCC choice for periodic words;
- TLC's multi-worker variance;
- timing.

## 5. Registered divergences the new core must honour

The default profile must keep each of these (`docs/divergences.md`,
`docs/tlc-differences.md`, `conformance/expected-divergences.toml`). Under tlc,
each one's TLC counterpart applies.

| ID | Behaviour | Core obligation |
|---|---|---|
| D-001 | Deterministic parallel search | G1, G5 |
| D-002 | Replayable randomness, seed reported | G3, F4; `RndTraceAssume` lost-trace behaviour kept |
| D-003 | Reproducible simulation | one stream per trace number (`dive-explore` simulation) |
| D-004 | Big integers by default | `Ctx` integer mode; `--int=tlc` reproduces 2178 |
| D-005 | Unreduced symmetry with liveness | no reduction in temporal runs |
| D-006 | Canonical `CHOOSE` and print order | D2 |
| D-007 | Exact storage option | exact key mode alongside fingerprint mode |
| D-008 | Structured results | JSON schema unchanged (ADR 0108) |
| D-009 | TLA+ comparisons | C1/C3 answered by TLA+; C2 kept |
| D-010 | Canonical exploration order | A3, A6 in the run's order; flag the stop |
| D-011 | Logical worker count 1 | G4 |
| D-012 | Independent counterexample validation | every trace replays in `dive-trace` |
| D-013, D-014 | Short deterministic lasso, periodic schedule | liveness interfaces (`07-engine.md` §7) |
| D-015, D-016 | Symmetry must be a permutation; audit (planned) | refuse non-bijections before search |
| D-017 | Host access needs explicit grants | grants also under tlc |
| D-018, D-019 | DFID depth-bounded zero exit; truncated cross-root witness | DFID only |
| D-020, D-021 | PlusCal checksums | not core |
| D-022 | Action composition with its TLA+ meaning | `cdot` modes; `=tlc` counts intermediate states, sequentially |
| D-023 | TLC exit statuses by default | section 2 table |
| D-024 | Historical Bits guard | none (historical) |
| D-025 | Reserved, never used | none |
| D-026 | Run budgets give a structured stop | budgets polled in the new loops; exit 5 |

## 6. Open questions for the owner

1. **Print count.** Should a difference in the number of `Print` calls be
   registered as its own D-entry (X2)? D-006 does not cover it. Today the
   reference engine already skips the replay's output.
2. **Default seed stability.** Default-profile draws depend on dive's state
   fingerprint (F4). If the new core changes that function, a reported seed
   stops replaying runs made with the old core. Is that acceptable, or must the
   key stay FP64?
3. **Value source provenance (E7).** Should the new core track where values
   were built, so that 2147 classes can be verified, or keep them unverified?
4. **Normalization outside the model (C4).** This needs a pinned probe: an
   out-of-model successor whose invariant tests membership of a record in an
   unnormalized set. Until then, should the new core refuse that shape under
   tlc?
5. **`TLCFP`.** `TlcExtTLCFP` is declared in `crates/dive-ir/src/std_op.rs:253`.
   I could not find its evaluator binding in `dive-stdlib` or `dive-eval`.
   Is it supported today?
6. **Symmetry with VIEW (X3).** Must the canonical representative be TLC's
   lexicographic minimum, or may it be any orbit-canonical function when the
   view is symmetric?
7. **History-dependent `elements()` (A6).** Keep the `NxInLazyCup` refusal,
   or track TLC's per-object normalization history to reproduce it?

8. **`ENABLED` cutoff (B3).** Is TLC's branch cutoff the correct TLA+
   treatment of substitution under `ENABLED`, which should therefore stay in
   both profiles, or a TLC artifact the default profile should drop like B2 and
   B4? Default until ruled: keep it in both profiles.