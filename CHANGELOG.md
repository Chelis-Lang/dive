# Changelog

- Register the existing default D-009 reflexive membership behavior of
  `ModelBadPrefix` as an exact model-specific expected divergence. Its exit
  and generated-count pairs are bound; TLC-mode results, goldens, trace checks
  and strict agreement are unchanged.

- Core temporal analysis can evaluate proved interval function construction,
  bounded interval CHOOSE and single-key scalar EXCEPT/@ inside ordinary
  action predicates and scoped non-stuttering checks. Exact original values,
  constructor demand, lexical scope and missing-key warnings remain guarded;
  hidden ENABLED queries and unproved updates retain reference evaluation.
  Eligibility counters alone do not establish speed or model completion.

- Correct initial-invariant whole-function errors to TLC's source-qualified
  fingerprint exception2147 after actual diagnostic receiver replay, in both
  execution engines. The underlying cause and counts are preserved; stored or
  unproved receiver lifetimes remain unverified.

- Preserve recorded initial fingerprint-error state context separately as
  optional `diagnostic_witness`, with no formal counterexample. Its existing
  native v2 named cause needs fresh ordinary-word and independent diagnostic
  replay before strict conformance can admit the deciding wrapper (ADR 0133).

- Reproduce selected initial tuple/record equality fingerprint wrappers after
  actual diagnostic receiver construction, including an Init-built receiver.
  Preserve D-009, native refusal boundaries, cached receiver lifetimes and owned
  interruptions; no source is inferred from error text or expression kind.

- Avoid temporary comparison-shape maps for bounded collections of closed
  integer-domain functions with matching scalar classes at each key. The
  existing comparison and normalization guards retain all other cases.

- Native trace version 4 adds independently replayed candidate-side terminal
  errors at constraint, action-constraint and implied-action sites (ADR 0128).
  Readers must implement the distinct final-candidate semantics and both primary
  and diagnostic replay before advancing producer pins; versions 1/2/3 cannot
  carry the new endpoint and keep their meanings. Outer reports remain schema 3.
  VIEW, unproved history and top-level non-Boolean role results retain explicit
  non-agreement. An actual source-qualified inner evaluator error, including
  `[A]_e` rejecting its non-Boolean action, can validate after both replay phases.

- Core temporal analysis can evaluate source-certified predicates and bounded
  fairness tests on exact native original rows using the existing compiler and
  a separate analysis Machine. Closed inputs, immutable interval domains and
  single-key scalar updates require actual domain/key checks. Unproved cases,
  unproved updates and semantic errors retain reference evaluation. Label demand,
  eager admission, attribution and independent replay remain unchanged.
  Additive atom counters report actual use; throughput still requires measurement.

- Human temporal-error reports print the same independently validated finite
  behavior as JSON. Capture statistics distinguish incomplete work from completed
  capture; uncertified diagnostic prefixes remain diagnostic only (#500).

- Native temporal capture stops retain the selected periodic schedule and the
  checks actually attempted before interruption. A stopped periodic run keeps
  `lncheck: deterministic`; final-only runs keep `lncheck: final` (#671).

- Reuse successful core-native temporal read conversions for exact persistent
  handles containing Bool/Int or flat tuple/record values. The bounded cache
  reuses shared rows for one-variable models, selects in-range row slots directly
  with an exact handle check, and charges its actual table
  capacity, row arrays and known value payload within the smaller
  of 128 MiB and 1/64 of the run memory budget; opaque Arc/allocator overhead
  remains explicit. Budget checks precede hits, eager admission still converts
  every original, and representation/history refusals remain unchanged. Cache
  counters are additive; conversion timing now excludes one-variable cache hits
  and reports its scope. Throughput and whole-run capacity need measurement.

- Reuse temporal read conversions for closed recursive functions, tuples and
  records with Bool/Int/run-owned-string leaves and scalar general domains.
  Exact representation handles keep literal classes distinct; all sets and
  unproved shapes bypass reuse. Charge nested payload and possible domain-order
  storage within the existing cache allowance. Every original still receives
  eager Graph conversion; this adds no capacity or throughput claim.

- The initial ordinary `--engine core` native temporal backend retained persistent original rows
  and CSR instead of a complete evaluator-value graph. Controlled eager Graph
  admission preserves representation/refusal order; analysis reads values on
  demand with zero retained Value cache at that checkpoint (#638). Native reports retain sealed complete
  or prefix facts and independently validated selected words, releasing native
  graph/store/plan owners before fresh replay. Rust reports add `NativeHolds`,
  `NativeViolation` and `NativePrefixViolation`; schema 3 and existing trace
  endings remain. Fixed capture counts and semantic refusals remain unchanged;
  complete memory accounting and TLC-relative capacity/performance qualification
  are still open.

- Ordinary core temporal files use actual run time/memory budgets and checked
  machine layouts across capture, products, acceptance and attributed words,
  removing inherited artificial count ceilings. Large native/product/search/word
  buffers grow fallibly and poll the existing budget. Genuine stops retain their
  stage, counters and capture-completion truth. Explicit Rust limits, reference
  routes and temporal-language/planning limits are unchanged; original capacity
  and performance qualification remain required (#650).

- Add a runnable CLI/Rust checking walkthrough covering safety success,
  independently replayed safety and temporal counterexamples, and a structured
  resource stop, using the existing Counter example.

- Low-level Rust producers can validate borrowed CSR adjacency and detach
  completed or committed-prefix evidence without constructing evaluator state
  values. The native core producer consumes this boundary; capture limits
  remain unchanged.

- Rust temporal analysis results can release their legacy capture graphs through
  `Analysis::into_native()` and `PeriodicAnalysis::into_native()`, preserving the
  owned candidate word and original complete or prefix producer facts. Candidate
  validation and existing reference entrypoints are unchanged.
- Liveness JSON, human and Rust reports name the same complete property list
  false on their actual independently validated word, using TLC list formatting.
  Conformance accepts different liveness name lists only after fresh whole-word
  validation and exact reported-to-word attribution; all other required fields
  remain strict (ADR 0126).

- Failed constant pre-evaluation retains the actual caller's lazy function
  selection: named functions no longer raise errors from undemanded entries.
  Successful constant caching and genuine demanded errors, resource failures
  and interruptions keep their existing behavior (#670).
- Core temporal graph conversion checks the capture budget during materialized
  traversal, normalization and sorting, with explicit scratch allocation errors.
  Low-level value constructors offer the same cooperative controls. Symbolic
  materialization and final shared allocations retain their existing behavior;
  capture limits remain unchanged.

- Refuse unproved non-EvalException Next primary classes when genuine temporal
  obligations remain (#502). Both capture engines keep actual counts/output and
  withdraw the old verified 2103/75 class and uncertified error word. Fairness
  syntax never selects GENERAL. Successful fairness and measured 2179 error
  controls stay unchanged; the reached fairness/replay pipeline remains open.

- Low-level Rust graph captures expose immutable producer facts through
  `evidence()` and release graph storage through `into_evidence()`. Completed
  and successful committed-prefix facts remain distinct. External graph
  factories preserve the original failure/stop verdict instead of admitting it
  as completion or a successful prefix (#624).

- Benchmark JSONL rows retain the child report’s actual engine, requested engine
  and fallback details, so campaign labels cannot hide reference-engine fallback.

- Admit pure native temporal tuple operations at all requested TLC worker
  counts when actual startup name domains are fixed. The sealed producer table
  and exact reference-domain guard prevent new-name demands from publishing a
  semantic result; source/configured/captured effects and foreign contexts remain
  refused. Reference-pool scheduling and original value representations are
  unchanged. This removes EWD998Chan's false TLC/8 static refusal without claiming
  original N=3 capacity or performance qualification.

- Report known persistent-store allocation components and capacities in core
  metrics, including unused slab slots, boxed payloads and retired index tables
  retained by readers. Label the total as a lower bound with explicit opaque
  allocation gaps; keep process-memory enforcement and graph limits unchanged.

- Fix default `--engine core` success for compound sets whose elements TLC
  cannot compare while normalizing (#619). Both profiles now report unsupported
  when the existing shape proof cannot establish safe normalization;
  `--engine auto` retries the reference engine.

- Extend core set-construction history to actual scalar/model-value/nested-tuple
  literal, MAP and covered union producers, including raw successor queries,
  nested EXCEPT, sparse normalized state handoff and replay. Preserve alias and
  first-representative behavior; unknown histories/error provenance still refuse.
  New bulk work uses the actual run budget, retaining actual stops after effects.
  Empty queries, repeated seen emissions, symmetry and nested tuple demand retain
  their existing supported behavior. This remains partial #623 coverage,
  without whole-model Sailfish performance qualification.
- `--engine core` consumes a bounded local Bool/inline numeral/tuple literal
  set lane in membership and `Cardinality`, with raw order, duplicate and
  alias normalization history retained. C2 normalization is checked in both
  profiles; default D-009 comparisons stay unchanged. Unproved comparison
  errors and wider producers remain unsupported (#623, still partial).

- Reuse exact comparison proofs for repeated compound membership misses in
  persistent sets under `--engine core --compat=tlc`, within the existing
  bounded memo allowance. Certified tuple proofs avoid temporary comparison
  maps, and model-value type checks use the worker's immutable name copy.
  General comparison guards stream constraints into one accumulator.
  `DIVE_CORE_OPT=-memo` disables proof reuse.

- Fix core counterexample reconstruction with `DIVE_CORE_OPT=-hashcons`: compound
  states now retain the invariant verdict and independently replayable word
  instead of an internal trace error (#603).

- `--engine core` evaluates `ENABLED`, including pure nonrecursive queries
  through `INSTANCE` constant/operator parameters whose variable mappings
  preserve root names. Hidden or renamed
  variables, initial queries, state-dependent configured operators and query
  effects/recursive/nested calls remain explicitly unsupported.
  BufferedRandomAccessFile runs on the core, about 7 times faster than on the
  reference engine.
- Fix `--engine core` checking of `ENABLED` inside action constraints and
  implied action properties: primed definitions now use the query's own
  successor, preventing false violations and missed action-property violations
  caused by reuse of the enclosing step's value (#588).
- Fix false and missed core invariant/action-property violations across nested
  `INSTANCE` substitutions (#634). Cached compiled definitions now distinguish
  each substitution context in ordinary and primed step checks, preserving
  existing query refusals and cache/alias options in both profiles.
- `--engine core` checks implied actions (`PROPERTY [][A]_v`, such as a
  refinement through `INSTANCE`) and action constraints faster: each
  definition or substitution they use is evaluated once per step, and once
  per distinct value of the variables it reads. EWD998PCal with its
  refinement property runs in 5.5 s instead of 14 s.
- `--engine core` accepts `Sequences!SelectSeq` under prime when its predicate
  depends only on its element parameter and constants. The sequence reads the
  successor context, including through `INSTANCE` refinement substitutions.
  State/captured/effectful predicates and other higher-order operators under
  prime remain unsupported in both profiles. The original `EWD998Chan` model
  now enters core search; its bounded admission probe establishes no completed
  verdict, capacity or performance claim.
- `--engine core` checks temporal properties (liveness, with fairness):
  the core explores and builds the behavior graph, and the reference
  engine's liveness checker checks it, so verdicts, counts and
  counterexamples are the reference engine's. `VIEW`, `SYMMETRY` and
  `Print` with a temporal property, and a liveness check that ends in an
  error, still fall back to the reference engine under `--engine=auto`.

- Low-level Rust temporal products accept checked borrowed CSR topology through
  `CsrTopology`, `Product::from_csr` and `with_csr_recurrences`. Edge order,
  duplicate edges and original witness positions are preserved. Checker graph
  retention now uses this topology; complete capacity qualification remains
  L5 work.

- Core graph capture delivers original accepted values for new and seen
  targets, retaining their representation and action/ordinal metadata. Temporal
  conversion preserves general-function versus tuple/record literal classes
  recursively through imports, folding and captured source frontiers,
  independently of report formatting. Error-word replay retains the separate
  canonical source identities. Parallel
  buffering uses the existing eight-source and per-source successor bounds;
  overflow and budget stops cannot become completed captures. Native compact
  state retention and larger graph capacity remain unimplemented.

- `--engine core` runs specifications that use `TLCSet` and `TLCGet` with
  integer or `"s:"` registers, `"all"`, `"all:named"` and `"level"`, with
  one worker and the values TLC computes. A run that writes registers and
  then ends in a violation or an error, `TLCSet("exit")`, and `TLCGet` of
  the counters or `"config"` still fall back to the reference engine under
  `--engine=auto`.
- Invariant evaluation-error counterexamples now retain the invariant name and
  underlying error cause. The independent validator and conformance harness
  verify that exact error in the final invariant instead of confusing its cause
  with TLC's deciding wrapper 2111. Earlier failures and forged names or causes
  remain invalid; other error-ending families remain tracked separately (#490).
  Migration: this new `invariant-error {name, ec}` ending uses native trace
  version 2. Readers must accept it and check its named cause before moving
  producer pins. All existing endings retain version-1 output and decoding;
  unknown versions/endings and a version-1 `invariant-error` fail closed. Outer
  check/validation reports remain schema 3 (ADR 0118).
- Independently validate named invariant errors and property violations whose
  final state or step lies outside model constraints. Constraint checks retain
  their configured order and stop at the first false result; earlier constraint
  errors invalidate a claim to a later safety failure. Repeated invariant errors
  retain their original name and cause in the existing native version-2 ending.

- Preserve the original named invariant-error word when a different error in
  diagnostic replay decides `TLC_NESTED_EXPRESSION` (#606). Native invariant
  site and first-report context remain distinct from the deciding class.
  Independent replay proves the original named cause; diagnostic class/counts
  and unclassified oracle outcomes keep their separate obligations. This uses
  existing trace version 2 and does not certify generic expansion errors.
- Fix reference checking of state substitutions used by native arithmetic,
  such as `Double == 2 * v` through `INSTANCE ... WITH v <- x` (#515).
  State-dependent bodies retain their caller's state instead of entering an
  empty-state constant cache. Constructor-bound values retain deferred-set
  cache identity and materialization history. Startup values, redirects, lazy
  demands and error order keep their existing behavior in both profiles.
- Independently replay pure terminal Next errors that occur before any successor
  is emitted (#434). A source-bound optional error endpoint uses separate fresh
  prefix and terminal runtimes, preserving exact cause/source and earlier-check
  priority. Reached composition is refused before intermediate-state collection
  in both modes. First candidates, unsupported effects and unproved sites stay
  refused; interrupted/resource-failed replay produces no validity report.
  This retains trace version 1 and adds no diagnostic-wrapper exemption.
- Preserve incomplete Next candidates as native trace version 3, with complete
  prefix rows separate from assigned/unassigned terminal bindings (#607).
  Logical word length includes the partial candidate. Fresh independent replay
  checks its exact bindings and generating source; earlier complete candidates
  remain unsupported in this slice. Versions 1/2 and report schema 3 retain
  their meanings; no missing variable becomes an ordinary value.
- Independently replay ordered pure terminal Next failures, including preceding
  complete candidates and the actual secondary diagnostic sequence (#434/#606).
  Exact source/cause/wrapper claims require execution; unproved global newness,
  alias history and value-source classes remain explicit unknowns. All ten
  original partial Next words now validate. Native version 3 also carries
  optional partial Init `error_witness`, keeping formal counterexample null,
  primary results and verified flags unchanged. No blanket wrapper exemption or
  TLC parity claim is added; trace versions 1/2 retain their meanings. Ordered
  replay preserves candidate/predecessor levels for `TLCGet("level")`, including
  earlier initial properties and generation resumed after a callback.
- `dive check` reports TLC's error when an operator whose body is a numeral
  or a string literal, such as `LAMBDA a, x : 0`, is passed to an operator
  TLC implements in Java (`FoldLeft` and the other folds, `Quantify`,
  `SelectInSeq`, `BagOfAll`, `SelectSeq`, `SortSeq`): TLC passes the value
  instead of the operator. It used to apply the operator (#557).

- `--engine core` runs models with both `VIEW` and `SYMMETRY`, evaluating
  the view on each state's canonical image as TLC does.

- `--engine core --compat=tlc` runs models with `SYMMETRY`, comparing
  states as TLC does. A comparison TLC could fail on is reported as
  unsupported, and `--engine=auto` runs the reference engine for it.

- `--engine core` runs specifications that use `RandomElement`, drawing
  what the reference engine draws at every seed: under `--compat=tlc`,
  TLC's own draws. The run uses one worker and `--seed` replays it. A draw
  in an invariant or constraint, a `LET`-bound draw used in more than one
  conjunct (TLC draws it again at each), and a draw in `Init` after
  constants or `ASSUME`s drew still fall back to the reference engine
  under `--engine=auto`.

- `--engine core` runs models with a `VIEW`: states are told apart by the
  view's value, and an error in the view is reported as the reference engine
  reports it.

- `--engine core` runs models with `ALIAS` as the reference engine does: the
  counterexample shows the states themselves, with a note saying so.

- `--engine core` runs `SequencesExt!ReplaceFirstSubSeq` and
  `ReplaceAllSubSeqs` on pure materialized sequences whose combined elements
  are all integers, all booleans or all strings, in both profiles. Mixed,
  nested, lazy or effectful sequence arguments remain explicit refusals.
  ReplaceAll also refuses overlapping occurrences after its early equality
  CASE arms, because TLC chooses by partition indices rather than absolute
  match positions. `--engine auto` reruns them on the reference engine (#610).

- `--engine core` runs `AllSubSeqs`, `LongestCommonPrefix`, and
  `ReplaceFirstSubSeq` and `ReplaceAllSubSeqs` on strings (SequencesExt),
  `FoldSet` (FiniteSetsExt), `FoldFunctionOnSet` (Functions), and `Len` of
  a string. `Len` of a string in an invariant used to end in
  an internal error on the core.

- `--engine core` scales better at high worker counts: interning a value
  that is already stored no longer takes a lock.

- `--engine core` runs specifications that use `ToString`, `Print` and
  `PrintT`, printing what TLC prints in TLC's order (with one worker). It
  now also reports what constant definitions and `ASSUME`s print at
  start-up, which it used to drop. A run that prints and then finds a
  violation or error, and a value whose printed form the core cannot know
  (a set that TLC may hold as an interval), still fall back to the
  reference engine under `--engine=auto`.

- `--engine core` uses less memory on models with many initial states or
  wide levels: MCBakery's peak falls from 769 MB to 136 MB at one worker.

- `--engine core` runs models with `SYMMETRY` in the default profile, with
  the reference engine's canonical keys, counts, warnings and notes. Under
  `--compat=tlc` it still refuses them.

- `--engine core` scales better on models that compare many strings:
  string comparisons no longer share a lock between workers.

- `--engine core` checks the safety parts of `PROPERTY` entries: implied
  initial predicates and actions (`[][A]_v`), as in a refinement property.
  A property that needs liveness checking is still refused (and `auto` falls
  back). EWD998PCal now runs on the core: 20 s against the reference
  engine's 80 s, on one worker.

- `--engine=auto` now runs `--compat=tlc` checks on the core too, and falls
  back to the reference engine where the core refuses. On the small
  conformance tier the result is the reference engine's own: 150 models
  agree with TLC and 6 are unsupported.

- `--engine core --compat=tlc` runs at any worker count (`--workers N`),
  with the one-worker result, as in the default profile.

- `--engine core` looks up functions on pair domains (`f[<<x, y>>]`) by
  position, and takes small set differences directly: GameOfLife runs about
  30% fewer instructions.

- `--engine core` reports a model that fails to bind (for example a numeral
  TLC cannot handle under `--compat=tlc`) as the reference engine does,
  instead of refusing it.

- `dive check --engine core --compat=tlc` runs TLC's profile on the new core,
  on one worker. It covers TLC's string order, 32-bit integers, `\div` from
  `Naturals` and TLC's `x' \in S` order. Comparisons TLC could fail on, and
  strings TLC first meets during the search, are refused, naming
  `--compat=tlc`. `--engine=auto` still runs `--compat=tlc` on the reference
  engine. `dive-conformance shadow --impl-profile tlc` shadows the two engines
  under `--compat=tlc`.

- `--engine core` bounds its memo tables: they start small, grow only
  while they evict, and stay within a share of `--max-memory`. With the
  tables on, MCLamportMutex at 8 workers drops from 308 MB to 185 MB, with
  no loss of speed.

- `--engine core` adds the default profile's D-010 note to a stop (a
  violation, deadlock or error), as the reference engine does. Its text
  output now equals the reference engine's on those endings.

- `dive check --engine core --core-sample N` compares the successors of `N`
  sampled states with the reference evaluator's and reports the result in
  the JSON report (`core.sample`); it changes no result.
  `dive-conformance shadow` samples 32 states per core run by default
  (`--sample N`) and reports a difference as `successors-differ`. CI's
  shadow gate now covers 26 examples models, and `cargo xtask gate --full`
  also shadows the cm1 list at 1, 4 and 8 workers.
- Successor sampling retains each source state's actual TLC level and restores
  it in both evaluators, fixing false sampler differences on
  `TLCGet("level")` models (#600). Reservoir selection and checked results
  are unchanged.
- Fixed: `--engine core` reported integers outside its inline 61-bit range
  as `<<>>` in counterexamples (#540).

- `--engine core` turns its memo tables back on by default
  (`DIVE_CORE_OPT=-memo` turns them off): MCLamportMutex runs about 29% fewer
  instructions with them.

- `--engine core` reports a new state that holds a set TLC cannot
  fingerprint (`Nat`, `Int`, an infinite `Seq(S)`, or a set whose
  fingerprint enumerates one, such as `SUBSET Nat`) as TLC does: the same
  error, class, exit code, counts and trace. It used to refuse the run.

- `dive-conformance shadow` runs dive's new core and its reference engine on
  a corpus selection and reports any difference in verdict, error class,
  exit, property, counts or depth, and any core counterexample `dive-trace`
  refutes. CI shadows ten examples models at 1 and 4 workers
  (`scripts/shadow-gate.sh`), and `cargo xtask gate --full` the small tier.

- Shadow comparison requires classified results and independently validated
  counterexamples for agreement. Matching unclassified or contradictory reports
  are findings, and an absent, unreadable or refused required word is a
  `trace-unconfirmed` finding (exit 1), with or without `--strict`. Classified
  evaluation errors can still agree; budget stops retain their timeout status.

- `scripts/fixture-shadow-census.py` accounts for every configuration fixture,
  uses seeded core/reference shadow comparisons, and keeps raw process output
  and complete outcome accounting outside the checkout.

- Failed `ASSUME` reports retain the evaluated assumption’s source image in
  `property`, matching TLC for anonymous and named assumptions (#504).

- `--engine core` reports a false or failing `ASSUME` as the reference engine
  and TLC do: the same message and class, and exit 10. It used to refuse the
  run.

- `--engine core` runs model values: constants assigned model values, sets
  of them, typed model values with TLC's comparison rules, and definitions
  the configuration assigns a value (`NoVal = NoVal`). The empty function
  `[k \in {} |-> e]` is now the same value as `<<>>` in the core.

- Fix `--engine core` on a specification whose initial predicate has
  several conjuncts (`Spec == Init /\ y \in S /\ [][Next]_v`). The core ran
  each conjunct as a separate initial predicate. It now conjoins them, as
  TLC does.

- `--engine core` preserves TLC's initial-state endings: generated candidates
  excluded by constraints still give depth one, while a truly empty Init
  gives depth zero. Retained initial states without a next-state action report
  model error 2115 (exit 75), even with deadlock checking disabled. Excluded
  initial states with no Next complete with zero distinct states and depth one.

- `dive-conformance run --impl dive:core` and `--impl dive:auto` run the
  harness on the new core. Reports record the engine requested and, per
  run, the engine that ran and any fallback (`summary.by_engine`), and
  baselines record a non-reference engine without making it incomparable.

- `--engine core` runs operator arguments, including `LAMBDA` and `LET`
  operators passed to operators. It also runs constant operators bound in
  the configuration (`F <- D`), and operator declarations that nested
  `INSTANCE`s substitute.

- Add `dive check --engine auto` (or `DIVE_ENGINE=auto`). It runs the new
  core where the core implements the check, and otherwise the whole check on
  the reference engine. The report names the engine that ran and the core's
  refusal. `--engine core` now refuses `--store exact`, disk spilling,
  `--chaos` and non-default evaluation options, which it used to ignore.

- `--engine core` runs actions that TLC splits inside a `LET`, such as
  `Pour(j, k) == LET amount == ... IN c' = ...`. Their `LET` values are
  evaluated lazily, as TLC does; before, the core refused them as "a split
  argument that is not constant".

- `--engine core` no longer enumerates `S` in `x \in UNION {S}`: MCDisruptor_MPMC
  runs about 40 times faster on the core.

- `--engine core` runs with `--workers N`. Each level is expanded in
  parallel and committed in order, so results equal the one-worker results.

- `--engine core` runs unions with an infinite side, such as
  `x \in Int \cup {NULL}` and `UNION {[D -> Int \cup {NULL}]}`. They stay
  lazy for membership tests, as in TLC.

- `--engine core` honours `--time-limit`, `--max-memory`, `--max-states` and
  `--max-depth` with the reference engine's structured `stopped` verdict and
  exit code. Before, it reported a time limit as a resource limit and
  ignored the others.

- `--engine core` reports evaluation errors and incompletely specified states
  as the reference engine does: the same class and exit code, the counts
  after TLC's call-stack replay, and the trace.

- `--engine core` reports counterexamples for invariant violations and
  deadlocks, rebuilt from a predecessor log by replay, in the same JSON trace
  format as the reference engine. Every one validates with `dive-trace`.

- `--engine core` now runs `<<A>>_v`, `UNCHANGED` of any expression, `x'
  \subseteq S`, functions applied as actions and `ACTION_CONSTRAINT`, reports
  TLC's warning 2143, and reports the queue at a deadlock or an initial-state
  violation. It now refuses `x' \in` a lazily represented union whose
  enumeration repeats elements, which M0 answered with wrong counts.
- Fixed: `--engine core` no longer answers `FALSE` to a comparison across
  kinds (`1 = "a"`, `{} = <<>>`, `"a" \in Nat`, `0 \in SUBSET S`). As
  with the reference engine, it is an evaluation error. Until the core
  classifies evaluation errors, the run is refused, as is a set whose
  elements mix kinds.

- Add `dive check --engine core` (or `DIVE_ENGINE=core`), the experimental M0
  spike of the new execution core (ADR 0117). It covers the default profile, one
  worker and safety only, and reports anything else as `unsupported`; it never
  falls back to the reference engine, which stays the default. `DIVE_CORE_OPT`
  turns strategies off for measurement without changing results.
- The experimental core (`--engine core`) now runs:
  - `INSTANCE` substitutions;
  - recursive functions and recursive `LET` operators;
  - TLC's lazy function and set values (for example `[k \in Nat |-> ...][x]`
    and membership in `{k \in Nat : P}`);
  - the Bags operators `EmptyBag`, `SetToBag`, `BagToSet`, `BagIn` and
    `CopiesIn`.

  It raises TLC's comparison errors instead of answering `FALSE`. Its memo
  tables are now off by default (`DIVE_CORE_OPT=+memo` turns them on).

- `dive simulate` now accepts initial states holding compound values and
  specifications with fairness conjuncts, in both profiles, instead of refusing
  them. Fairness cannot affect a finite safety sample. Configured temporal
  properties still refuse (ADR 0116, #491).
- Read the native-hook mode without taking the evaluator's host lock on every
  standard-operator call; callback hooks still run exactly as before.

- Check temporal properties under any `--store`, `--fp`, `-deadlock` and
  `--sequential` setting. Plain `dive check --compat=tlc` (random polynomial),
  `--store exact`, `-deadlock` and `--sequential` previously refused every
  liveness property. Remaining route exclusions now name the blocking option.

- Refresh the unchanged 265-model full-tier measurement to 182/217 agreements
  (+1), with `SpanTreeTest5Nodes` as the sole gain and no lost agreements.
  Retain 16 refusals, ten resource stops and nine missing references; the
  generated digest and recovered originals do not constitute a full-gate pass.

- Pin two generated primary references distinguishing a stored startup definition
  from a configured redirect to its effectful body: success versus a temporal
  violation with an unnamed property. No runtime change or new native agreement
  is included; established conformance populations and baselines are unchanged.

- Learn generic closed powerset-filter restrictions from original empty and
  singleton evaluations, preserving whole predicates and accepted sink order.
  Position skips require the current operative host policy and unchanged checked
  depth; unknown contexts retain ordinary enumeration.

- Preserve TLC's zero-size powerset BASE boundary: yield its empty subset before
  component conversion, retaining the actual size check, nonempty errors and
  initial invariant witness. No error-class mapping or admission rule changes.

- Pin four empty-subset-base primary results, preserving initial invariant
  violations with unreported counts and distinct mixed-comparison/size-overflow
  details under their actual error envelopes. No runtime fix, baseline change
  or new agreement is established.

- Preserve GENERAL as the primary class of source-qualified Init errors while
  retaining diagnostic replay exit 75. Carry native causes through typed,
  nonreused deferred-error receipts; rendered text grants no provenance.
  Register the observed nested Cardinality case under the approved D-004
  big-integer default, with exact exit and generated-count pairs.
- Make native permission-hook execution an explicit host binding policy for
  fresh owned checking runs. Legacy/custom and replay hosts keep callbacks;
  other permissions, startup state and control remain unchanged. This adds no
  predicate mask, new enumeration route or performance claim.

- Add eight pinned primary references for nested powerset Init enumeration,
  including original candidate order and error boundaries. All six earlier
  result payloads remain identical; no enumeration algorithm, limit or baseline
  changes.

- Pull concrete nested-set filter candidates through the existing Init,
  successor and `ENABLED` continuations in original order. Required component
  sizing/normalization and predicate errors/effects remain before their actual
  outputs; explicit Cardinality and bulk enumeration keep their existing routes.

- Classify two source-verified TLC Init exception/replay envelopes without
  promoting nested causes or relaxing context-only failure checks. Preserve
  GENERAL1000 and the actual exit75; malformed, unowned and unknown exceptions
  still refuse golden generation.

- Report TLC startup error 2253 before assumptions and exploration when the
  actual combined temporal-role formula normalizes to no solutions. Preserve
  prior binding output and null counts without evaluating discarded state atoms;
  effectful or unproved conversions retain explicit refusals.

- Pin eleven additional startup temporal-error priority references, including
  combined-property and conversion-output controls, before implementing error
  2253. Existing goldens and established conformance populations are unchanged.

- Record the private eight-cell function-membership census. It establishes no
  qualified repeat successes or instruction savings; provider-excluded work
  remains unknown, and no membership cache is implemented.

- Record complete frozen-main instruction and caller attribution, including a
  bounded EXCEPT cost query. Selected contexts do not justify a 5% rewrite;
  no runtime change or throughput improvement follows from the evidence.

- Avoid the runtime-name sequential restriction for concatenations whose complete
  checking sources prove a tuple operand, using constructor-grounded result
  facts without assuming type invariants. Other creators, effects and unresolved
  roots keep their restrictions; evaluation and limits are unchanged.

- Record the rejected resolved-call ownership screen: instructions increased
  by 2.021% geometrically across all eight pairs, with six cells exceeding
  the 1% regression ceiling. The private runtime prototype remains unmerged.

- Add certified current-state temporal row errors for eligible single-worker
  source-bound requests, preserving actual error order, prefix statistics and
  fresh finite-path and independent ordered error-endpoint validation. Trace1
  endpoints use fallible validation transport; resource exceptions produce no
  validation verdict. Unproved contexts retain the old route;
  caller-owned model APIs, limits, scheduling and report schema stay unchanged.

- Record the rejected ready explicit-set payload screen: all eight pairs
  reduced instructions, but the 2.018% geometric reduction missed the fixed
  5% bar. The private runtime prototype remains unmerged.


- Record complete frozen-main evaluator cost attribution and the measured
  rejection of hash deferral before implementation. Checker behavior,
  conformance populations and baselines are unchanged.

- Record two pinned temporal atom demand references: startup tautology rejection
  and compiled-bin error priority. Both remain unsupported in ordinary
  comparisons; established populations and checker behavior are unchanged.

- Record the rejected invocation-session evaluator screen: all eight pairs
  increased instructions, by 73.271% geometrically, with matching counters.
  The private runtime prototype remains unmerged.

- Add an opt-in W1 capture checkpoint API retaining reached Init/source-row
  evidence and typed causes without enabling temporal atom evaluation, changing
  ordinary verdicts or claiming new agreement.

- Explain unresolved semantic names and operator symbols with concrete scope,
  declaration and module-import guidance. Preserve producer-specific help,
  original diagnostic evidence and analysis acceptance.

- Refresh the generated full-tier TLC-profile baseline from the complete
  one-worker measurement: 181/217 in-scope agreements, ten gains and no lost
  agreements. Preserve 17 refusals, ten resource stops and nine missing
  references, with explicit post-run packaging recovery provenance.

- Record the rejected general native-argument storage screen: all eight pairs
  reduced instructions, but the 0.254% geometric reduction missed the fixed 5%
  bar. The runtime prototype remains unmerged.

- Record two rejected evaluator instruction screens: transient activations
  increased instructions by 0.573%, while compact raw outlining reduced them by
  0.441%, below the fixed 5% bar. Both runtime experiments remain unmerged.

- Retain original typed state-atom causes and actual analysis sites in detailed
  Rust temporal failures, preserving `DIVE-L0033` refusals, verdict policy,
  scheduling, counts, limits and the public report schema.

- Add four pinned primary temporal atom error timing observations for issue
  #451, including a preserved SANY rejection, and a separate angle-action
  error-order reference. Established parity populations and checker behavior are unchanged.

- Prove non-string result kinds from owned sources during bounded SelectSeq
  callback analysis, allowing canonical native Concat past its own runtime-name
  check. Preserve both argument walks, unused captures, provider/effect checks,
  source errors, native authority and all existing limits. String or unknown
  result kinds retain their named refusal.

- Add seven pinned SelectSeq/Concat capture and error-order references before
  callback type-capability changes. Preserve exact source metadata and existing
  parity populations; this import changes no runtime admission policy.

- Add opt-in `golden generate --raw-setup-status` alongside `--raw-invocations`
  to retain separately versioned, bounded setup subprocess receipts and original
  streams. Censored or unrecordable setup stops before later dispatch; normal
  nonzero capability results retain their existing checks. Defaults, model raw
  records, golden schemas and model-pass scheduling are unchanged.

- Borrow captured behavior topology during temporal analysis instead of retaining
  a second adjacency and root copy. Preserve captured states, concrete successor
  occurrences, public graph APIs, validation order and all resource limits.

- Add pinned first-state compound-domain oracle controls and independently
  validated counterexamples for issue #349.
- Stream unassigned compound-membership candidates over concrete components,
  reaching initial/successor stops and ENABLED witnesses without retaining the
  complete powerset, product, function set or record set. Existing normalized
  bulk enumeration and deferred-provider routes retain their contracts.

- Add opt-in `golden generate --raw-invocations DIR` to retain original TLC
  clean/trace streams, invocation commands and process outcomes, including
  partial output on failure. Golden schemas and oracle pass scheduling stay
  unchanged; Java setup, capability probes and SANY are outside this option.

- Retain the original inner source expression for finite callback admission
  failures through aliases and lazy actuals, preserving the existing refusal
  class, message and admission policy.

- File temporal requests in both profiles use the finite machine-index maximum
  for reachable product vertices and report the actual numeric ceiling. Existing
  Rust defaults and explicit custom ceilings remain enforced; all other owner
  limits and native budget semantics are unchanged. The report stays at major 3.
  CoffeeCan1000Beans now agrees in both profiles; the complete small tier is
  150/156 in TLC mode and 144/156 in the default, with zero disagreements.

- Stream unassigned `ENABLED` interval-membership candidates in ascending order,
  stopping at the first witness. Rejected membership candidates reach the existing
  cooperative budget checkpoints; assigned-variable membership remains unchanged.
- Stream primitive interval columns in bounded evaluator contexts. Evaluate all
  binder domains before the first body, preserve first-variable-fastest order
  and tuple-error priority, and retain cooperative loop checkpoints without
  first allocating the interval. Preserve original cardinality admission
  errors in both profiles. Consumers that request all contexts still collect
  the complete batch. Other domain materialization remains bounded
  only by its existing cooperative resource contract (#349).
- Apply the armed run budget to producer `ASSUME` evaluator loops in safety
  checking and behavior-graph capture. Interruption reports `stopped` during
  `check-startup`, preserving prior output without a graph or search verdict.

- Retain conformance child-group leaders unreaped through cleanup on Linux and
  macOS. Serialize cancellation with spawn registration and reject new work
  after cancellation or an unproven drain. Preserve native output, exit,
  signal and timeout classifications; report process-inspection failures as
  harness errors. Enforce the existing Linux/macOS host scope before launching
  children elsewhere (ADR 0112, #430).

- Store coordinator product adjacency in private flat target and original-edge
  arrays with canonical row ranges over the original discovery buffers. Remap
  destination IDs in place. Preserve reachable-pair discovery, ordered
  witness projection, public owned products and every existing resource limit.

- Qualify file graph-label admission on the unchanged 156-model small tier:
  149 TLC agreements; default 143 agreements and six expected divergences,
  including MCCRDT's registered D-022 counts; seven named refusals each.
  All six affected checks complete and all eight normal instruction cells stay
  within 1% overhead. Historical full-tier and four-worker results keep their
  original sources and counts.

- **Report migration:** every public operation now emits report major 3.
  Temporal settings replace ambiguous `atom_cells` with explicit graph-label,
  attribution-row and word-truth owners and report actual acceptance policy.
  Readers must migrate before moving their pin; supported immutable report-1/2
  inputs and native dive-trace v1 remain unchanged. Advance notice: Oyster #70.
  No DFID/simulation outcome tags change (ADR 0111).
- Add explicit graph-label policy to new final/periodic `*_with_policies` APIs.
  Existing Rust APIs preserve every custom finite ceiling. File checking removes
  only the graph-label count ceiling; attribution, word, capture, tableau,
  product and acceptance bounds remain. Native budgets retain their documented
  cooperative scope; no hard allocation-time memory guarantee is added.

- Fallible function-domain lookup retains a typed evaluator interruption for
  functions with 32 or more keys, instead of turning it into the existing named
  comparison refusal. Ordinary comparison errors and evaluation order are kept.

- Fresh model binding can carry cooperative evaluator interruption through
  constant startup evaluation, role binding and action expansion without
  swallowing it or reporting an unsupported feature. Check requests and fresh
  native temporal-candidate rebinding use the same owning run budget and retain
  actual startup output/capture evidence. Additive controlled Rust binding APIs
  preserve existing uncontrolled calls. Structural/bulk startup, assumptions,
  post-binding symmetry and legacy session validation remain outside this change
  (#349); no count, default or report-major change. Exact-source full qualification
  passes all 2,243 tests and both small baselines; every normal single-worker
  instruction cell stays within 1% overhead (issue #425).

- Default-profile conformance recognizes only the exact documented D-022
  generated-count pairs for MCCRDT and MCReplicatedLog. Matching runs remain
  expected divergences; other counts, verdicts, exits, depth and trace facts
  remain checked. TLC profiles and adapters receive no exception (#413).

- Specs using `Json!ToJson` now retain the existing reported sequential worker
  fallback under TLC value or exploration order, preserving generated-name
  encounter order. Default ordering and named Array/Object refusals are unchanged.

- Successor domain candidates and action-composition intermediate right steps
  observe run budgets even when their continuation emits no successor (issue
  #393). Interrupted private composition collection does not invent generated
  states or a completed graph; earlier deciding errors and sink stops retain
  their priority. Binding, assumptions and quiet/value-only bulk units remain
  outside cooperative interruption (#349, #379).

- General finite functions share immutable domain metadata across eligible
  same-context EXCEPT updates, use exact integer/Cartesian lookup certificates,
  and borrow checked domain views. Existing unknown-key comparison/refusal,
  literal/order, hash, fingerprint and codec behavior is retained. A complete
  reduced eight-cell screen measured 5.365455% fewer instructions geometrically;
  the full checker gate and both small baselines pass without regressions.
  This is no wall-clock or TLC-throughput claim.

- Live action-membership metadata uses iterative summaries of the original
  formula DAG instead of retaining copied normalization trees. Preserve original
  action IDs, Boolean simplification and exact direct atomic GF/FG classification,
  including repeated junction operands. Input/depth/work budgets remain enforced;
  no default limits, classifier rules or tableau algorithms change (#387).

- Benchmark campaigns pass explicit TLC-derived time budgets and process-memory
  ceilings to every native arm. The external backstop defaults to 900 seconds;
  native resource stops retain their details and never earn completed timings.

- Native temporal candidate certification registers the owning run control on
  fresh independent replay, including labels, `ENABLED` and fairness. Both the
  supplied word and native serialized round trip must validate; interruption
  reports `stopped` at `trace-validation` with real capture and optional
  validation progress, never an uncertified violation or feature refusal.
  Add generic `dive_trace::validate_controlled` with typed interruption outside
  the unchanged validation verdict tags. General TLC interpretation/session and
  standalone CLI budgeting, binding/assumptions, serialization, quiet value
  units and deferred providers from other evaluators remain outside this scope.

- Post-startup evaluator quantifier, CHOOSE and filter loops cooperatively
  observe run budgets across nested evaluation (issue #374). Interrupted
  evaluation retains typed control through deferred sets and reports `stopped`
  with actual search/capture/analysis progress; it is never a false predicate
  or a capacity refusal. Binding, assumptions, diagnostic error/count and path
  reconstruction after an already deciding event, fresh independent validation,
  and value-only bulk/native work retain their explicit interruption limitations
  (#349, #379). Rust callers matching evaluation or
  value error classes must handle the additive `Interrupted` variants.
  Fallible bag/function comparisons preserve typed interruption; quiet
  infallible equality/order/hash units defer control checks to preserve truth
  and cache identity, and remain an explicit uninterruptible value unit (#349).

- Finite memory budgets sample before binding and before a holding result,
  including short checks in both profiles (ADR 0109). macOS uses safe native
  RSS observations. Initial known exhaustion stops at `check-startup`; unknown
  RSS or default-ceiling RAM is a typed `DIVE-R0002` error, never silently
  unlimited or zero memory. Add truthful sampling metadata without changing
  report schema-2 verdict tags. Rust `Budget::poll` is now fallible; owners
  finalise their watchdog once and preserve earlier errors/validated witnesses.

- `dive check` and `dive tlc` accept opt-in `--max-states N|none` and
  `--max-depth N|none`. Initial states have BFS depth one; both defaults and
  TLC-equivalent settings are `none`. The first new in-model identity that
  would exceed a ceiling stops before admission, with resource `states` or
  `depth`, actual partial statistics, no counterexample or completion claim,
  and exit 5. Exact-bound finite graphs can still finish. Parallel generation
  retains its worker pool and publishes only the admitted reference-order
  prefix. Every stopped report with statistics marks them `exhaustive: false`.
  Rust callers get optional `max_states`/`max_depth` request fields and new
  `budget::Resource::States`/`Depth` variants; exhaustive matches must handle them.

- Unassigned initial/successor interval domains stream ascending integers to
  their sink instead of materializing the whole range before the first state
  (issue #349). Endpoint, ordering, error and big-integer semantics are retained.
  Parallel successor emission also observes the armed run budget (issue #362),
  preserving prior ordered errors/violations and actual incomplete progress.

- The model gates and nightly conformance checks use explicit TLC-derived time
  and memory budgets in both profiles (ADR 0107), replacing fixed per-model
  timeouts. The harness retains strict comparison for its exact budget-only
  compat override; exhausted budgets remain resource findings.

- Requested run budgets now fail explicitly with `DIVE-R0001` and exit 4 if
  their watchdog cannot start. No check runs with the limit silently disabled.
  Rust callers must handle the new `Result` from `Budget::start` and `budget::arm`.
- Liveness atom labels pack truth by its actual state or action domain,
  sharing original-edge offsets instead of allocating unused false rows.
  Both profiles retain the same predicate evaluation order, source errors,
  count bounds and independently validated counterexamples.

- `dive check` runs under a wall-clock and a memory budget (ADR 0107, D-026):
  `--time-limit DURATION|none` (30 minutes by default in the dive profile,
  provisional until release; none under `--compat=tlc`) and `--max-memory
  BYTES|auto|none` (half of physical memory by default in both profiles,
  measured as the whole process's resident memory on Linux and macOS). An
  exhausted budget stops safety search, graph capture or temporal analysis
  with `verdict: "stopped"`, a `stopped` block (resource, limit, observed
  value, phase, hint), the statistics reached so far, and exit status 5 under
  both exit conventions. The JSON additions, including `options.budget`, are
  additive within schema versions 1 and 2. Rust callers use
  `CheckRequest::{time_limit, max_memory}` or `CheckOptions::budget`. No count
  limit changes. `dive tlc` accepts `-Xmx8g` or `--max-memory` for the memory
  ceiling, which applies under `--compat=tlc` too (a host-resource exception,
  like D-017); an explicit `--time-limit` there labels the profile
  `tlc+overrides`. The conformance harness keeps its timeouts and passes
  `--time-limit none --max-memory none` unless `--tlc-budgets` is given, which
  derives each model's budget from TLC's recorded time (`max(60 s, 10 × TLC)`,
  `--budget-cap` default 900 s, never below twice TLC's time). For Oyster's
  pilot and other strict consumers: the new keys are additive, and `stopped`
  appears as a verdict only when a budget runs out; Oyster's runs under its
  own short wall-clock limits are unaffected unless they exceed half of
  physical memory or, in the dive profile, 30 minutes. Both `run` and
  `study final` count time and memory stops as resource findings, rather
  than unsupported features.
- Temporal atoms and fair-action binder domains can call pure state-level
  top-level `RECURSIVE` operators. Admission checks semantic levels, effects
  and binding scope; bounded binders, `LET` and recursive function values use
  ordinary evaluation. Independent counterexample validation supports the
  same atoms. Action/temporal bodies, `ENABLED`, effects and unproved dynamic
  bindings retain named refusals. Both profiles match TLC's evaluation-leaf
  rule; the checked three-state fixture produces 7 generated states.

- Temporal admission no longer enumerates constant-domain predicate binders or
  refuses them at 4,096 bindings. Quantifiers, maps, filters and nonrecursive
  function constructors are inspected once in both profiles; runtime evaluation
  and independent trace replay retain concrete bindings. Formula/storage limits
  still bound actual temporal expansion.

- Liveness checking separates pure recurrence and persistence conditions
  from temporal memory, sharing a tableau/product across strong-fairness
  alternatives. Every alternative remains part of the check, finite prefixes
  remain unrestricted, and counterexamples retain independent validation.
  Both profiles use the same semantic decomposition; existing count bounds
  are unchanged. Due completed-level checks inherit the armed run budget on
  their capture thread, preserving stopped outcomes and incomplete progress.
  Explicit acceptance policies count shared scratch bytes, condition inspections
  and scratch initialization alongside the base acceptance buffers and work.

- Temporal SelectSeq admission inspects pure bounded callback binders and
  INSTANCE captures without enumerating domains. Nested selections share one
  source-inspection budget, and local recursive-function captures retain their
  self bindings. Runtime evaluation and independent replay keep original scopes;
  unsafe or opaque unused captures still refuse. Referenced definition value
  overrides also refuse before recursive-leaf or native-source shortcuts.

- Liveness checking evaluates its atoms with the core evaluator and its
  definition and lazy-value caches, instead of a bounded re-evaluation that
  first walked every binder domain (issue #322, ADR 0103 follow-up). On the
  small tier `LoopInvariance/MCQuicksort` and `YoYo/YoYoAllGraphs` now agree
  in both profiles; no other result changes. Checking the liveness models
  there executes 75–89% fewer instructions and runs up to 27 times faster
  (for example `MCFlashWithMutexLive` 978 s to 36 s, `ReadersWriters` 118 s
  to 14 s). An atom evaluates only the branches it reaches, as TLC does.
  `dive-eval`'s `EnabledResolver`, pure-predicate budget and finite-binding
  preflight API (`eval_scoped_pure`, `check_finite_bindings` and related),
  `Host::reuse_expression_cache` and the `enumerate` cargo feature are
  removed.

- Ordinary `dive check` in the default profile now checks eligible temporal
  properties at deterministic completed-level boundaries (D-014, ADR 0018 §7)
  instead of only once at the end: after a BFS level, once the graph has at
  least 2^16 distinct states and has doubled since the previous check. A
  violation found on such a prefix stops the run with that level's actual
  counts and an independently validated lasso; only complete exploration
  reports success. Models below 2^16 states are checked once, at the end, as
  before. Reports record `lncheck: deterministic` and the schedule in
  `periodic`. `--compat=tlc` and `--lncheck final` keep final checking. Rust
  callers can set `CheckRequest::periodic`.

- Add 35 generated TLC references for full-tier models that had none: 27
  from a 600-second pass and eight larger models from four- and eight-hour
  passes. Nine full-tier models still have no reference
  (`docs/conformance-corpora.md`).
- Evaluation allocates less per operator call (ADR 0104, item 2): a frame
  keeps up to four slots inline in its own allocation, an operator slot is
  boxed so that a slot is no larger than a value, reading a bound name no
  longer copies its slot, and an argument that is a literal or a name bound to
  a value is bound directly instead of through a thunk. Results are
  unchanged; single-worker checking runs 6–11% fewer instructions on the
  benchmark models in both profiles.

- Membership of a function in `[S -> T]` decides `DOMAIN f = S` without
  building the domain set when `S` is an interval, an enumerated set or a
  product of those, in both profiles, comparing the same elements in the same
  order so that answers and errors are unchanged (ADR 0104, item 4). Type
  invariants such as `x \in [0..N-1 -> T]` and `childOf \in [Nodes \X Keys -> T]`
  run 4–14% fewer instructions on SimpleRegular and MCbtree.

- Self-hosted CI runners never let `rust-cache` cache or prune
  `CARGO_HOME/bin`, and each runner uses its own `CARGO_HOME`
  (`docs/developer/ci.md`).

- Values free their contents without allocating a work list in the common
  case: children that are scalars or shared are released in place, and nested
  values are freed by bounded recursion before falling back to the work list,
  so any nesting depth stays safe. Results are unchanged; single-worker checking
  of the benchmark models executes about 3–7% fewer instructions
  (geometric mean about 5%).

- Temporal atoms no longer fail with a TLC evaluation error raised by a binder
  domain that evaluation never reaches, such as a `CHOOSE` without a witness
  or a two-argument function applied outside its domain behind a false guard.
  The finite-binding preflight visited every branch and reported the error; it
  now skips that binder, and evaluation still reports the error (or refuses,
  where dive does not reproduce TLC's message) wherever TLC fails.
  `YoYo/MCYoYoPruning` now agrees in the TLC profile.

- Temporal graph capture no longer refuses a specification as random merely
  because it extends `Randomization`: that module's own helper
  `TestRandomSetOfSubsets` calls `RandomSetOfSubsets`, and every IR node was
  scanned. Only definitions outside the standard modules, and what they reach,
  now count. `SpanningTree/SpanTreeTest4Nodes` now agrees in the TLC profile.

- CI jobs can run on self-hosted `dive-local` runners by setting the
  repository variable `DIVE_CI_RUNNER` (see `docs/developer/ci.md`).

- The `dive` and `tlc` binaries use mimalloc as their global allocator, and
  release builds use fat link-time optimisation with one codegen unit (ADR 0102).
  Results are unchanged; single-worker checking executes about 15–17% fewer
  instructions. Building from source now needs a C compiler.

- Final temporal analysis validates all retained bound-model contexts before
  planning can evaluate constants or invoke native hooks. Malformed public
  metadata refuses during planning without starting graph capture.

- `dive-conformance run --baseline <file>` fails on corpus regressions against a
  committed baseline: a model or module that agreed and no longer does, or a
  changed verdict, error class, exit code, property, count or depth in a model
  that does not now agree.
  `report baseline` writes the compact baselines in `conformance/baselines/`.
  `cargo xtask gate --full` and a nightly job check the small tier in both
  profiles against them. Run records now carry the error class and violated
  property.

- Ordinary `dive check` in the default profile now checks eligible temporal
  properties with the bounded final analyzer and independent lasso validation,
  as `--compat=tlc` already did, instead of refusing them. Reports record
  profile `dive` and `lncheck: final`; the deterministic periodic schedule
  (D-014) remains planned.

- Trace validation decides `ENABLED`, deadlock endings and weak or strong
  fairness with the core evaluator's `ENABLED` instead of a separate
  finite-cover search (ADR 0103). Fairness whose action guards a primed read,
  such as `ack' = IF x' = 1 THEN ~ack ELSE ack`, now validates instead of
  refusing with `DIVE-T0020 ... no finite cover` (issue #321). The
  `dive_trace::exists` API is replaced by `dive_trace::enabled::enabled`.
  Replay reuses only lazy values it computed itself (its own cache domain), so
  a value cached by ordinary evaluation, for example through action
  composition, never becomes a validation verdict. `enabled::enabled`, and
  `ENABLED`, `WF` and `SF` in `temporal::evaluate_lasso`, refuse action
  composition by name as replay does. Public typed states sorted
  in the other profile's value order are refused with `DIVE-V0043`. The
  `fairness-replay` corpus adds Oyster's weak and strong pilot monitors.

- Default TLA+ membership comparisons borrow unnormalized set elements instead
  of cloning them into temporary storage. Written-order comparisons and TLC
  fallback errors are preserved.

- Bounded pure query evaluation owns its lazy-expression memo, preserving
  captured lexical contexts and binding lifetimes during independent replay.
  Source-ordered partial assignments retain a LET across precisely its enclosed
  choices. Recursive lazy functions retain shared work through a weak memo
  reference; deferred providers and exhausted entry/work bounds refuse explicitly.

- Lazy argument caches in named ENABLED queries retain pinned TLC's root-name
  inclusion rule across hidden candidates. Independent replay support for the
  cache-dependent query remains unfinished (issue #313).

- Record pinned hidden ENABLED lazy-cache controls before implementing
  compatibility support (issue #313).

- Native operator callbacks retain the enclosing `ENABLED` lookup flag while
  clearing other callback control flags. Primed reads through `INSTANCE`
  substitutions now preserve their cutoff, preventing both missed initial
  invariant violations and false violations in `SelectSeq` callbacks.
  Independent replay retains these query scopes, gathers primed callback
  dependencies and requires finite covers before evaluating actual candidates.

- Add a separate primary-oracle corpus for action-valued lazy sources in
  INSTANCE enabledness callbacks, with constant and current-state controls.
  These fixtures establish the oracle behavior for issue #312; native checking
  and independent replay support remain separate requirements.
- Record primary ENABLED lazy-binding lifetime controls, including genuine and
  fabricated invariant endings (issue #313).

- IR child visitors avoid constructing temporary child lists in structural
  validation, frame metadata and effects analysis. Existing owned child lists,
  source order, admission checks and error priorities are preserved.

- Golden generation refuses unclassified TLC/SANY output and oracle errors
  before creating or overwriting a reference, including with `--reason`. These
  results report generation failures; classified specification errors remain
  valid references, and existing timeout/resource-limit refusals are preserved.

- Temporal planning and independent word validation distinguish an absent
  INSTANCE substitution from an explicitly unset binding. Original variables
  and configured constants remain available through absent substitutions;
  pure zero-argument constant redirects retain their real scopes. Strict
  holding and violation controls retain actual counts and reject forged words.

- Independent finite-cover queries retain hidden assignments by canonical name
  and follow actual INSTANCE/lazy scopes. Ordinary counterexample predicates
  dispatch reached ENABLED queries to this search, including queries inside
  data-valued lazy arguments, while preserving ordinary host and error order.
  Query decisions from another evaluator invocation cannot satisfy replay through
  a thunk cache. General callable/temporal INSTANCE integration remains open.

- Evaluator ENABLED searches retain hidden successor assignments by canonical
  variable name, preserving exported/nested INSTANCE cutoff and lazy-scope
  distinctions in both profiles. Named-state reads, cache compatibility and
  priming retain actual assignments; ordinary graph states remain root-only.
  Independent INSTANCE query-word validation and general callable/temporal
  integration remain unfinished, so this adds no corpus agreement claim.

- Benchmarks retain multi-worker TLC's reported depth as a diagnostic under
  ADR 0004, while keeping exact depth for single-worker TLC and every native
  arm. Missing/zero depth and all other required observables still fail validation;
  JSONL records whether depth equality is required.

- Added a separate ten-model pinned INSTANCE/ENABLED namespace corpus before
  runtime implementation, documenting exported-query, hidden-name and captured
  binding distinctions. Existing corpus totals and native support are unchanged.

- File-based final temporal checking now bounds compact acceptance buffers by
  8,000,000 requested bytes and independent 32,000,000 materialization work units.
  Existing public owned/custom-limit APIs retain legacy logical cells; additive
  policy APIs expose byte/work selection. This does not bound process memory or
  search work, and leaves other graph/evaluator limits unchanged. Byte mode can
  reuse original behavior-edge recurrence truth across product transitions, with
  checked product-domain fallback and explicitly charged metadata, initialization,
  projection/write and Boolean work; parallel edge positions remain distinct.

- First-order temporal expression forwarding retains existing direct ENABLED
  queries; forwarded operator captures and query data remain refused.

- Sequential graph capture now combines runtime-name encounter order with TLC
  action-composition accounting without misclassifying those ordering needs as
  stateful checking. Real effects, parallel capture exclusions, checkpoint/DFID
  restrictions and independent composed-word refusals remain in force.

- Temporal analysis stores acceptance in compact bit columns, reducing retained
  Boolean storage. Public owned APIs and logical resource limits are unchanged.

- Finite one-worker simulation supports one direct scalar Init `RandomElement`
  choice with separate startup and worker-selection streams. Normal domain
  errors and Print output are retained; returned words require independent
  membership validation. Other reached random calls and caller-host startup
  continuity remain refused. Broader simulation support (M/#239) remains open.

- Ordinary TLC-profile bounded final checking also accepts the existing
  `--action-composition=tlc` option with otherwise default resident settings.
  Original intermediate generated counts and independent composition refusals
  remain; a counterexample is published only after independent validation.

- Deferred finite-filter binding can resolve scalar value-bound constants in
  otherwise pure configured redirect sources. This bounded, callback-free
  metadata inspection uses the existing domain work/depth limits; predicate and
  captured-context effects remain conservative. Definition/operator-bound or
  deferred constants and recursive/higher-order/captured targets still refuse.
  Definitely forbidden global effects refuse before charged inspection, even
  with no remaining work, preserving their original error-class priority.
  Two tiny filtered-WF references establish hold/fail controls; original
  Quicksort sufficiency and broader corpus coverage remain unmeasured.

- Pure temporal state/action predicates support direct configured first-order
  user operators with lazy scoped arguments. Native, captured, substituted,
  recursive and higher-order configured targets retain explicit refusals.
  Independent trace replay and finite-binding checks inspect the actual body.

- Final temporal action labeling uses the first original successor for each
  destination identity at each source, preserving constructor-sensitive errors
  and all generated counts. A single direct EA/AE action operand can report its
  actual evaluator `GENERAL` error as TLC code 1000 / exit 255. Synthetic
  nonboolean errors and uncertain compound or multiple-property error ordering
  remain unsupported. Canonical state labeling and residual-tableau validation
  refusals are unchanged. Bounded constant simplification excludes eliminated
  action operands only at graph labeling. Existing eager attribution and
  independent validation can still refuse eliminated-action candidates; those
  refusals emit no model error or counterexample. This additional action-only
  normalization can itself exhaust the existing formula limits.

- Bounded temporal predicates and fairness actions can retain pure finite local
  recursive-function scopes, including actual self and lexical/state captures.
  Independent enabledness keeps original complete-cover semantics and refuses
  hidden primed reads even behind empty domains. Configured/higher-order and
  effectful recursive contexts remain unsupported; existing limits stay in force.
  Recursive results not established constant still refuse as temporal quantifier
  domains, including closed calls; state-level analysis is conservative.
  Original allocator/corpus qualification on this source and full gates remain pending.

- TLC checking entrypoints now read and parse the configuration before the
  module, so missing or malformed configurations exit 151 even when the module
  is also missing. Native commands and existing file APIs retain their input order.

- Recognize the exact pinned Hanoi signed-AND class by SHA-256 with checked
  four-IntValue admission, configuration priority and checkpoint class identity.
  Unknown local classes refuse before startup evaluation can execute their TLA
  fallbacks; configuration substitution and arity errors retain their priority.
  The revised checker candidate uses the maintained one-worker reference at
  each supported native width. DirectTLC4 differences remain historical; fresh
  source qualification and complete gates are required before acceptance.
  No comparator, golden or ExpectedDivergence exemption changes.

- Reject benchmark goldens whose semantic oracle, ordered classpath or module-
  library pins differ from the configured inputs, before launching a timed child.
  Historical Java/platform details and physical paths remain allowed metadata.

- Repair manual ten-model benchmarking with matched TLC worker counts, paired
  per-repetition wall ratios and scaling, explicit incomplete-pair counts, and
  effective configuration and executable provenance. CoffeeCan safety-only
  work is named separately. Benchmark timing and performance targets remain
  unmeasured by this runner repair.

- Eligible ordinary TLC-profile checking now reuses bounded final temporal
  analysis with deterministic short, independently validated periodic words.
  Configured safety roles preserve their single startup binding and schema-1
  results; actual temporal reports use schema 2 with complete capture counters.
  The ordinary conformance adapter checks physical status and coherent metadata
  and independently validates words. Distinct counts, depth, property and error class stay
  strict; generated counts are required at one worker; valid periodic lasso choice and queue population may differ. Raw queue
  data and final-capture exhaustion consistency are retained. Native ordinary temporal
  checking, excluded options, unsupported features and resource limits retain
  explicit refusals; periodic scheduling and full TLC parity remain incomplete.

- Added checking-only `dive tlc`, installed `tlc`, and argv[0] `tlc` entrypoints
  with TLC evaluation, one worker by default, module/configuration basenames,
  common checking flags and independent validation before emitting a word.
  `-lncheck final` selects the existing bounded deterministic short-lasso backend
  directly and reports its qualifications. Tool messages and TLC simulation,
  DFID, storage and debugger protocols remain named refusals. Native command
  and file API defaults remain unchanged. Full TLC argument and tool-output
  compatibility remains open.

- Add bounded `SVG!PointOnLine` with the installed override's signed32 wrapping,
  binary64 interpolation and truncating/saturating result in both profiles.
  Unimplemented argument representations remain named refusals; SVG layout,
  serialization and animation completion remain unsupported.

- Low-level simulation refuses overlapping/reentrant calls on a shared evaluator
  core before evaluation, preserving each run's output limits and caller host.
  Separate bound cores remain independent; cleanup restores the host before
  releasing ownership.

- Ordinary single-worker finite simulation evaluates runtime `Print`/`PrintT`,
  including captured operators (#239), with retained event/text limits. Binding
  and backend output remain separate; fresh independent validation preserves
  return/error semantics while discarding replay output before sink callbacks.
  Reached random outside the supported Init subset, registers and other host
  effects remain refused. This does not complete simulation, generation or schedulers. The accepted `none`/`basic`
  action-statistics behavior is retained.

- Explicit final/short checking without configured `PROPERTY` entries now runs
  ordinary safety checking and returns its real result without duplicate startup
  evaluation or a temporal holding/capture claim. An experimental narrow
  default-memory route also checks configured properties that bind entirely to
  safety roles using one checker-owned binding and its original startup host.
  Other options, transformed randomness and real temporal obligations retain
  their existing analysis route and bounds. Safety-only recursive property
  evaluation retains the ordinary resource limit; genuinely temporal recursive
  operator captures retain their named planning refusal. Current-source qualification and
  independent review remain pending; no corpus agreement is added. Pre-search binding/preparation model
  errors retain the binding stage; engine errors retain the capture stage.

- Added bounded pure `Json!ToJson` conversion with Gson string escaping and the
  existing canonical default order. TLC object histories, representation-dependent
  Array/Object entry points, unbounded domain printing and JSON file operations
  retain named refusals. Independent trace replay accepts already-interned JSON
  strings; newly created runtime names retain the existing named refusal.

- Storage regression coverage now includes literal-tuple and integer-domain
  function comparison through forced frontier/predecessor spill and checkpoint
  recovery, alongside the existing application and independent-witness controls.
  The storage guide documents structural format 2 and its separate trace contract.

- PlusCal source mappings retain real statement and macro call-site byte spans
  across CR/LF/CRLF and mixed source endings. Rewrite metadata reports the actual
  emitted `cr`, `lf` or `crlf` separator without changing generated bodies or CRCs.
- `dive check` now uses TLC physical process statuses independently of its
  evaluator profile. `--exit-codes native` retains the historical CLI statuses;
  existing Rust `CheckReport::exit_code()` callers are unchanged. New typed
  convention methods and JSON/human reports disclose the selected status and
  mixed compatibility. This is the new explicit ADR 0097 decision; earlier
  numeric-exit accounting measurements remain historical.
- PlusCal TLC compatibility now returns replacements for modified or unverified
  translation bodies like pinned pcal Noop; the default protects them unless
  `--replace-modified` is selected. Reports expose requested/effective editing
  settings and retain previous body checksum mismatches.
- PlusCal generation recognizes bare CR and CRLF physical lines while preserving
  original byte spans; an unavailable algorithm CRC does not gate valid output.

- Final temporal predicates and independent trace replay support authoritative
  `Sequences!SelectSeq` with binder-free pure state user/LAMBDA callbacks and
  known forwarded lexical captures. Each real invocation checks actual elements
  in sequence order under a shared atom budget, including enabledness search
  and nonstuttering comparisons. Independent ENABLED/fairness checks retain
  complete finite covers and lazy pure state helpers. Bounded predicate
  evaluation bypasses caller definition/thunk result caches. Action, binder, dynamic, provider, effectful
  and configured/native predicate shapes retain named refusals.

- Conformance now compares physical numeric exits exactly in ordinary and
  worker-specific paths. Known default differences require declared integer
  TLC/native pairs and count as ExpectedDivergence, never Agree. Native CLI/API
  statuses are unchanged. Required whole counterexamples cannot be omitted
  or replaced by metadata-only traces, even in worker-specific comparison;
  independent replay uses the same selected golden as comparison. Historical
  default agreement totals omitted this
  observable and are qualified in the scoreboard pending complete remeasurement.

- Private trace validation supports one direct single-variable initial equality
  `x = RandomElement(S)` over a nonempty interval or stored finite scalar set.
  It checks exact domain membership and the complete claimed ending, independently
  of the sampled result. Startup, correlated and later draws remain refused.
- Replay bypasses caller-owned lazy argument and function result caches, so a
  warmed checker result cannot evade independent composition refusal.
- Independent trace validation refuses reached action composition before
  intermediate-state enumeration even in a feature-unified workspace. Public
  finite-cover and lasso APIs retain the same independent refusal boundary.

- Action-composition controls retain compact generated oracle results and compare
  deciding verdict classes and depth alongside generated/distinct counts.

- Trace API and CLI validation reconstruct a private startup runtime from
  immutable source/config inputs. Startup registers and deterministic
  single-candidate Init membership are supported without caller cache/register
  reuse. Registers after Init, other random draws and other host effects remain
  explicit refusals; raw bound-model validation remains conservative.
  Randomness under VIEW or SYMMETRY retains the shared source/config refusal
  before either ordinary API binding or private startup evaluation.

- Trace mutation controls now select their shortest-BFS or general
  counterexample contract explicitly (#259). The corpus harness reserves
  dropping and swapping states for single-worker exhaustive finite witnesses,
  while simulation, multi-worker and liveness controls retain the applicable
  value, claim and label mutations. Every generated mutant must still be
  rejected directly and through its document; an empty suite has no negative
  coverage credit. Existing `SpecModel::controls` and `mutants` remain
  documented shortest-BFS compatibility methods.

- Independent Init validation now uses startup level 0, while generated-state
  invariants and implied initial properties retain level 1. Register access after Init
  and random draws remain refused.

- Final temporal checking preserves the binding stage's TLC error class and
  exit status, including invalid `_PERIODIC` definitions. Valid periodic hooks
  retain their named refusal.
- Action composition in DFID now accepts the composition-only sequential BFS
  reason and counts emitted successors, matching pinned TLC; effect restrictions
  remain. Disabled BFS composition has a verified TLC error class and exit.
  Standalone evaluator tests exercise the independent non-enumerating refusal,
  and CLI tests cover explicit mode and JVM-alias precedence.

- The independent trace validator is a plain replay checker (ADR 0016,
  amendment 3). Under `--compat=tlc` it validates as usual and reports
  `unsupported: runtime string encounter order` only when decoding or
  checking the trace created a name after the model was bound, so models
  whose sequence operators only build tuples (for example
  `MCCheckpointCoordinationFailure`) now validate. Random draws in a check are
  refused by name instead of replayed. The `--trace-format representation-v2`
  flag and trace format version 2 are removed. `trace-controls` and
  `SpecModel::controls` retain corpus mutation QA without random redraws. TLC JSON interpretation scoring
  now evaluates only pure predicates, preventing register writes from making
  invalid initial states validate, and leaves untaken effects unevaluated.
  Register reads and writes during replay are refused by name because they
  depend on off-trace evaluations; validation leaves caller registers intact.
  Public typed trace, lasso and enabledness states now reject foreign name
  tokens and opaque deferred values before evaluation.
  Malformed public predicate roots and source locations report unsupported
  before document decoding or replay instead of panicking in diagnostics.

- Rescue reset (ADR 0086): parity is defined by observable results. Vendored
  evidence trees, per-increment docs and evidence checker scripts are removed;
  tests keep only the fixtures they read. PlusCal and storage notes are merged into
  `docs/pluscal-translation.md` and `docs/user/storage-recovery.md`, coverage is
  summarized in `docs/scoreboard.md`, and a pre-commit repository budget guard
  (`scripts/check-repo-budget.py`) is added.


- Add source-owned stopped-worker periodic diagnostics through
  `periodic_candidate::check_optional_stopped`. Eligible ordered source errors
  remain authoritative across diagnostic callback failures and timer/pool joins;
  no failed source becomes EOF, final checking or temporal Holds. Ordinary
  scheduling remains refused; the executable guide records actual method/CLI
  evidence and the unproved Java after-error graph/count boundary.


- Explicit final TLC mode now admits pure state conditional predicates through
  the corrected shared planner and fresh independent witness replay. Scoped
  LET/INSTANCE, entry/closing and error controls retain all earlier refusals;
  resource bounds and ordinary periodic scheduling restrictions stay unchanged.


All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased

- Add `--symmetry-liveness=unreduced|refuse|tlc` for checking and independent
  trace replay (D-005). The dive profile disables symmetry reduction for the
  entire run only when actual bound roles include genuine temporal properties;
  safety-only checks retain their existing reduction. Reports retain the declared
  symmetry and requested/effective policy. Explicit unreduced TLC evaluation is
  `tlc+overrides`; TLC quotient liveness and VIEW remain named refusals.
  The default-profile P4 count exception binds the exact full count tuple and
  reported unreduced mode; it is expected divergence, never agreement.


- Explain five semantic argument-shape families: operator/function argument
  counts, required operator arguments, and operator/LAMBDA arity. Analysis reports
  fill absent help while retaining producer suggestions and diagnostic evidence;
  semantic acceptance and the report version are unchanged.

- Parser reports now supply concrete help for every assigned syntax diagnostic,
  preserving more specific parser suggestions. `dive explain DIVE-S...` covers
  the same codes without changing parser acceptance, spans or report versions.

- Repository tasks select the invoking Cargo workspace at runtime, so a cached
  gate driver cannot silently check another worktree (#402).

- `dive explain <CODE>` provides human/JSON causes and concrete remediation for
  the assigned temporal, temporal API, watchdog and command-adapter codes.
  Temporal failure reports retain their actual cause spans and add help;
  stage-wide failures keep unavailable spans null. Catalogue coverage is
  explicit, and unknown codes return no guessed explanation.

- All public JSON operation reports now use schema 2, including ordinary safety
  checks and trace validation. Check reports add `check_kind` to distinguish
  safety/temporal report families without claiming execution. Consumers must
  accept validation report 2 before updating their pin; native dive-trace remains
  version 1. Report/trace compatibility and migration guarantees are documented
  in the reports guide (advance consumer notice: Oyster #61).

- Finite predicate preflight streams non-tuple quantifier bindings through one
  copy-on-write context, while retaining all domain, visit and error checks.
  Tuple patterns, analysis scopes and public eager binding APIs are unchanged.
  Runtime benefit and original-model sufficiency are not yet measured.

- Bounded final temporal analysis supports nested pure ENABLED queries in
  Boolean action guards and fairness, with independent finite-cover validation
  of original Next and closing edges. Existing limits and explicit refusals for
  data/provider/higher-order/substituted/primed query placements remain in force.
  Original-model qualification and full gates remain pending.


- Finite simulation accepts `--stats basic` and
  `SimulateRequest::action_stats`, returning sparse counts of consecutive chosen
  actions grouped by predicate source location. Counts include Init provenance
  and stutters and retain the chosen prefix on failures. Unused value-bound
  invocations preserve their names and locations; configured definition targets
  retain their action bodies. Sampling, seeded replay
  and independent safety validation keep their existing contract. This does not
  complete probabilistic generation, schedulers, DFID or expression coverage.

- Remove the empty `dive-compile` and `dive-lsp` workspace placeholders and their
  unused dependency permissions. Language-server and native-compilation work
  remain on the post-v1 roadmap (ADR 0095).

- Make PlusCal algorithm CRCs best effort while always preserving the real body
  checksum. Retire procedure header-transfer emulation and checksum prerequisites
  for rewriting/debug generation. Changed algorithm text no longer needs an
  overwrite grant; edited body protection remains explicit. Translation reports
  use schema2 without proof fields, retaining actual analysis and option settings.

- Preserve the resolved PlusCal input version when selecting the first algorithm
  in a comment, including legacy `fair` followed by prose or another algorithm.


- Add bounded PlusCal debug AST locations and exact Full-header CRCs for simple
  procedure-free, macro-free shapes. `dive translate --ast` and public original
  AST APIs return the earlier semantic serialization in memory, with artifact
  availability separate from translated-module acceptance. Original procedure/macro
  ownership and compound locations now consume genuine pre-generation WriteAST
  controls. Eight debug compound/macro headers match pinned CRCs and preserve body
  bytes. Procedure-free labelled If/Either/While headers retain their measured CRCs.
  A separately measured procedural body remains exact; its algorithm CRC is
  deliberately unavailable after header-transfer retirement. Compound/process-owned procedures,
  renaming, simultaneous ordering, historical fields and actual `-writeAST` file
  operations remain unfinished; SANY warnings grant no clean checksum validation.

- Expose measured PlusCal `-termination`, `-noDoneDisjunct`, `-lineWidth`,
  sequential version semantics and distinct `-reportLabels` through the CLI,
  source-option resolver and public API. Generation and optional exact checksums share
  effective settings. `-noold`/`-nocfg`/`-unixEOL` report honest in-memory artifact
  preferences; no files are written. Broader debug/original-stage AST shapes,
  historical process/procedure shapes and specification-backed translators remain named
  unsupported. The corpus tool can transport actual `-label` to both translators.

- Replace PlusCal translation admission by a port of pinned `pcal.trans`
  semantics: scoped renaming, multiline macro substitution, branch labels,
  symbolic process domains and shared procedure frames. Macro amplification is
  bounded before token allocation; expanded depth and generated output are
  bounded during construction. Existing translation blocks are replaced at
  their original location; whole-module analysis and real generation limits remain
  explicit. Native source maps and macro locations are preserved.
- PlusCal corpus measurement verifies pinned inputs and compares complete native
  check reports for both translations without crediting refusals or malformed
  error classes. `--help` succeeds, and relative `--dive` paths are resolved
  before entering scratch directories. These are translation checks, not
  independent TLC model parity.

- PlusCal measurement rejects omitted statistics and generated counts below
  distinct counts without turning legal explicit-null startup reports or empty
  Init counters into refusals.

- Graph capture uses an inert-by-default `capture::Observer` with initialization,
  edge and completed-level events at one worker. Callback failures preserve
  actual boundary counters and return incomplete exploration. Producer-clock
  requests, dispatch-pause receipts and stopped/terminal services are removed.
  Safety, capture and checkpoint calls share one search entry; DFID reuses
  concrete-value extraction and role evaluation while retaining its own search
  algorithm. State keys refuse missing assignments instead of using an empty
  vector, and invariant-error replay retains the resolved winner keys without
  evaluating VIEW again (#241).

- Report the pinned GENERAL startup error class for division by zero in a pure
  symmetry declaration and incompatible values while normalizing its set.
  These errors precede ASSUME and exploration, exit 255 under `--compat=tlc`
  (default model-error exit 1), and preserve other startup refusals.

- A model with no bound action now ends after Init without checking
  POSTCONDITION, as TLC does, and the depth is 1 when constraints exclude
  every generated initial state (#231). PlusCal wraps a long multi-procedure
  `Next` disjunct at the pinned translator's 78-column width. Test temp
  directories are claimed atomically, so a recycled PID cannot collide with
  an interrupted run's files.

- Remove the TLC-internal liveness emulation (#241, ADR 0082): TLC tableau
  numbering, phase-L receipts, SCC visitation, witness selection, the periodic
  controller, hook, lifecycle, service, snapshot and terminal diagnostics, the
  `dive_api::tlc_temporal` and `dive_api::periodic_candidate` APIs, the
  `TemporalMode::FinalTlc` mode, the `--lasso tlc` CLI spelling (now a usage
  error) and the `dive-final-tlc` harness mode. `tlc_plan` shrinks to
  `classify`, TLC's 2213/2214 formula refusals for `--compat=tlc`, and
  `periodic_backend` to the `Growth` trigger. A configured `_PERIODIC` entry
  now binds with TLC's errors (2229, 2228) before it is refused. Final/short
  checking (`--lncheck final --lasso short`) remains available through one
  shared planner. Numeric temporal operands such as `<>1` now retain TLC's
  evaluation error instead of reporting an unsupported feature, including
  code 2215 and exit 255 under `--compat=tlc` (13 in the default profile).
- Evaluator, model and checker admission scaffolding is removed (#241). Disk
  seen-state checking now binds PROPERTY and SPECIFICATION roles like the
  resident route and gives its results, instead of refusing effectful,
  quantified, strict-substitution or fairness-only roles. Randomness combined with
  VIEW or SYMMETRY uniformly refuses before binding or search. Ordinary VIEW and
  non-VIEW native random streams remain available, including configured targets.
  Constant values are no longer size-limited at evaluator construction. DFID
  and parallel graph capture refuse effectful models through one conservative
  effects check instead of per-scope purity proofs, including actual operators
  supplied in public Rust predicate/action scopes before any backend work.
  Pure captured operators remain supported. Unused value-admission context error
  variants are removed. An evaluator now refuses
  any scope created by another evaluator. The unused deferred-postcondition
  admission token and its observer entry point are removed after the periodic
  pipeline deletion.

- Remove the obsolete `dive_trace::Replay` type and ignored replay arguments
  from the trace-validation API. Ordinary random evaluation remains explicitly
  unsupported; direct initial support uses independent relation membership.

- Action composition (`A \cdot B`) is evaluated by default, with its TLA+ meaning (D-022).
  `--action-composition=tlc` (or `-Dtlc2.tool.impl.Tool.cdot=true`) reproduces TLC's
  experimental implementation exactly; `--compat=tlc` keeps it off, as TLC does.
  `dev capture` takes `--feature`.
- CI runs three lean jobs (authorship, a single-compile `Workspace gate` with
  the nextest `ci` profile, and an eight-model conformance smoke at 1 and 4
  workers) instead of twenty. `cargo xtask gate --fast` mirrors it locally;
  `cargo xtask gate --full` runs the whole suite, the CM1 and validation-smoke
  gates and the small corpus tier, and writes `target/gate-report.md`.
  `conformance.yml` now runs on manual dispatch only (#241).

- Symmetry reduction compares a candidate set by selecting its elements in
  order, with each element's images under every permutation cached per thread
  and ranked, instead of building and sorting every permuted set (#242).
  `SimplifiedFastPaxos/FastPaxos.cfg` drops from 162 s to 20 s of CPU at one
  worker under `--compat=tlc`, and from 570 s to 17 s in the default profile,
  with unchanged counts. Records now count as comparable field by field, so
  sets of records with differently typed fields normalize by the total order.
  A set mixing a record with a function that is neither a record nor the
  empty tuple now normalizes through TLC's checked insertion sort, so it
  raises the comparison error wherever TLC does; it was sorted silently.

- Prevent lost notifications in the test-only returned-prefix rendezvous; retain
  its five-second bound, actual timer/request controls and default behavior.

- Independent trace context inspection admits bounded closed Strict literals
  and actual constant forwarding before entering their scopes (#234), retaining
  pre-effect refusals for forged calls, lazy bindings and ordinary evaluator
  semantics. Both original temporal regression tests are preserved.


- Strict independent trace replay can prove bounded finite tuple ranges from
  Init and every Next branch, admitting indexed process stacks without using
  witness values as namespace/type evidence. Unproved branches, strings, opaque
  maps and effects retain named refusals; arbitrary-state APIs and budgets stay
  unchanged. Executable CLI/Cargo guides cover both transports and profiles.

- Source-owned pre-startup admission extends opt-in disk seen-state checking to
  proven pure safety PROPERTY/SPECIFICATION roles through the shared binder.
  Temporal/effectful/opaque and evaluation-demanding contexts remain explicit
  refusals; resident binding, actual inline fallback and separate budget scopes
  stay unchanged.

- Composed request/candidate diagnostics, disk safety and positioned replay,
  bounded procedure checksum editing, and inductive tuple replay with fresh
  release verification and maintained user/developer guides.

- Add private actual graph/tableau/resource finalization and terminal outcomes to
  the owned periodic candidate diagnostic. Final selection precedes direct posts;
  counterexample render errors remain separate, and unproved getter/partial
  paths refuse. Original source winners, bounds and independent memory/JSON replay
  remain authoritative; ordinary scheduling and Holds are still unsupported.


- Add a source-owned exhausted-queue terminal diagnostic before timer/pool joins. Request-lane setup/join failures retain actual initial or returned-prefix statistics. Genuine finished queues skip configured hooks; deciding final words reuse fresh independent replay. Ordinary checking and scheduling/count/model-proof claims stay refused.

- Add source-owned optional periodic diagnostics: genuine configuration absence skips only the hook stage, while mandatory APIs, pre-startup guards and private independent candidate replay remain unchanged. No ordinary scheduling or model-proof credit.

- Add a bounded producer-owned temporal candidate Rust API: original source values and product selection are reconstructed privately, then freshly rebound for independent in-memory and serialized JSON replay. No caller witness/Valid authority, ordinary schedule verdict or model-completion credit.

- Bounded Full PlusCal checksums support one unfair finite process calling one
  disjoint labelled procedure. Source-owned process call vectors stay separate
  from sequential LAST aliases and indexed runtime state (ADR0079). Global
  resource limits, invalid-reference admission and strict saved-frame replay
  boundaries remain unchanged; runnable C/P and Cargo examples document them.

- Bounded Full PlusCal checksums admit source-finite self/mutual sequential
  procedure graphs and preserve pinned top-level/shared nested CallReturn
  headers (ADR0077). Unchanged shared resource limits and explicit artifact
  admission keep primary-invalid tails and execution/witness claims separate.
  Runnable C/P and Cargo-owned examples describe the remaining scopes.

- Bounded Full PlusCal checksums support disjoint acyclic sequential procedure-body
  calls, preserving each owner's source call vector separately from the reference
  last-header main alias (ADR0073). Actual recursion remains a named refusal;
  shared budgets, CRC policy and strict saved-frame witness qualifications remain
  unchanged. The guide includes executable C/P and Cargo-owned Rust examples.

- Bounded Full PlusCal editing supports disjoint explicitly labelled sequential
  procedures called from main. Typed call frames retain target ownership while
  the original header preserves the last declaration's shared call vector
  (ADR 0069). Call graphs, process-owned checksums and strict saved-frame witness
  replay remain separate named boundaries; quotas and defaults are unchanged.

- Bounded P `elsif` sequential-procedure Full checksums retain virtual Else
  ownership and conditional continuation updates, with pinned stored-header
  byte/CRC controls and unchanged resource budgets (ADR 0066). Artifact, model
  and strict saved-frame replay qualifications remain separate.

- Full PlusCal editing derives exact header AST/CRC for proved nested sequential
  procedure If/With/Either/While transfers, with bounded typed expansion and
  separate artifact/model/replay qualifications (ADR 0061). Unvisited While TRUE
  tail assignments preserve reference creation order; split/comment token-line
  conditions retain the conditional stored-header path. Public multiline
  procedure generation remains named Unsupported.
- Bounded Full PlusCal editing now derives exact header AST checksums for one
  disjoint labelled sequential procedure, with ordinary calls/returns and
  call-goto. This earlier increment retained nested transfers as refusals; the
  later bounded nested continuation above lifts only its proved shapes. Wider
  ownership remains unsupported; artifact admission and model proofs stay separate.

- Independent whole-word replay can prove bounded tuple-preserving variable
  histories from Init and every Next branch; unknown history remains refused.
- Strict independent whole-word replay admits proved tuple-only `Tail`,
  concatenation and `SubSeq` operands while retaining string-history and
  variable-stack refusals; includes a runnable CLI/Cargo guide.
- Trace JSON documentation now distinguishes tagged value kinds and document
  round trips from construction-preserving value transport and independent
  validation. Mixed record/tuple set decoding limitations remain tracked in #190;
  this clarification does not change admission or validation behavior.

- Source-owned pre-startup admission extends opt-in disk seen-state checking to
  proven pure safety PROPERTY/SPECIFICATION roles through the shared binder.
  Temporal/effectful/opaque and evaluation-demanding contexts remain explicit
  refusals; resident binding, actual inline fallback and separate budget scopes
  stay unchanged.


- Opt-in bounded disk seen-state safety membership uses fallible ordered inline
  admission, preserving concrete outcomes while reporting actual worker fallback,
  positioned errors and explicit cleanup. Graph/recovery and unproved
  Print/PrintT invariant-error reconstruction stay refused; pure replay uses
  positioned fallible membership/insertion with frozen generated counts.

- Add a fallible private-file seen-state library for full fingerprints and canonical exact keys, with bounded disk indexing/rehash, first-error poisoning and explicit audited cleanup. Ordinary checker admission remains pending (ADR 0063).
- Structural storage format 2 keeps integer-domain tuple functions distinct from
  tuple literals in frontier spill and native recovery. Mathematical keys and
  fingerprints stay unchanged; older value-bearing archives refuse explicitly.

- Opt-in `--trace-format representation-v2` preserves admitted stored mixed
  sets and nonliteral tuple/record functions with checked construction witnesses.
  Legacy version1 stays default; transport is not a semantic certificate and
  provider/symbolic/unproved shapes retain explicit bounded refusals.

- Closed pure ordinary VIEW/RandomElement checking seeds from the transformed
  FP64 value while retaining concrete witnesses for independent replay. Stored
  shape/work admission and existing startup, first-successor, symmetry,
  Randomization-family, graph/temporal and RNG recovery refusals remain explicit.

- Periodic backend runtime-fraction arithmetic matches 196 actual pinned updater
  observations, including wrapping and NaN/zero behavior. It remains separate
  from ordinary scheduling and checker verdicts.


- Add a bounded request-owned periodic Rust diagnostic: real prefix append precedes a single configured-hook/controller service, including empty requests. Separate receipts retain actual counters and source identity; no ordinary temporal verdict or default/profile change.

- Producer-owned request pauses publish the maximal returned contiguous source prefix through the shared safety/graph path, retaining later buffers without regeneration. Typed receipts distinguish graph publication from physical frontier append; no ordinary temporal verdict or schedule credit. See `docs/committed-periodic-prefix.md`.

- Producer-owned progress requests pause actual expansion claims and join admitted work under real dispatch authority; privately retained receipts keep uncommitted output separate from logical/temporal graph publication. Runnable Rust diagnostics grant no ordinary schedule or model-proof credit.

- Add a bounded configuration-owned periodic snapshot Rust diagnostic with prebinding startup admission, original role/body identities and exact ordinary-check refusal. It grants no ordinary schedule or temporal proof.

- Bounded pure supplied periodic expressions compose at real native pauses, retaining original comparison/evaluation errors. Configured `_PERIODIC` and ordinary temporal CLI remain refused; see `docs/periodic-hook.md`.
- Native commit-boundary lifecycle diagnostics use real borrowed dispatch authority, internal clock/controller polling and actual EOF final SCCs. No ordinary temporal verdict is enabled; see `docs/native-periodic-lifecycle.md`.

- Source-owned periodic backend diagnostics retain pinned pointer-bucket growth/check-size snapshots, private joined-wave facts and first-candidate stopping. Original work bounds and independent replay remain required; ordinary periodic checking and partial Holds remain unsupported. See `docs/periodic-backend.md`.

- Bounded partial temporal SCC/source-checkpoint diagnostics retain written-record authority and candidate fields under shared graph limits; successful partial checks never certify temporal completion.
- Fixed-arity `Append` operator arguments in explicit bounded final TLC checking,
  with independent operand and witness proof. Tail/SubSeq runtime-name refusals
  and existing budgets remain unchanged.
- A closed complete stored-successor final-TLC census retains all 140 models:
  60 agree, 75 explicitly refuse and five time out. Historical source/binary
  receipts and every raw outcome remain separate from later producer results.
- Explicit final TLC state admission accepts stored Enum/Raw sets, including
  normalized sets nested in tuples, records and general functions. Callback-free
  namespace/order checks and unchanged cumulative work/depth bounds precede
  fingerprinting; cached deferred providers still refuse.

- A periodic backend controller preserves pinned queue/force/PERIODIC/checkpoint
  callback order, with all 296 selected Java observations and typed backend
  failures. It does not enable ordinary temporal verdicts or certify actual clock
  authority and checkpoint persistence.



- Memory VIEW preserves nonforcing normalization of evaluator-owned lazy filters; unknown providers and unproved lazy-operation caches remain explicit refusals. Spill and checkpoint deferred-value admission is unchanged.



- Bounded global PlusCal LOCAL and function DEFINE declarations preserve helper
  visibility, domains and pre-state lookup in both profiles. Domain/body
  action/temporal and recursive/name conflicts remain named refusals; artifact
  admission, CRC matching and finite model proof remain separate reports.
- Bounded PlusCal DEFINE helpers preserve global/local declaration order and
  pre-state helper references, with exact post-translation AST/Full CRCs.
  Recursive, action/temporal and unproved scoped declaration shapes refuse
  explicitly; public generated modules still require syntax/semantic admission.
- PlusCal embedded selection follows pinned line-first marker ownership; a later
  unfair algorithm cannot replace an earlier fair one. Same-line priority and
  split/tab/space separators have paired primary controls (#192).
- Bounded nonrecursive PlusCal macros support Full post-expansion AST CRCs.
  Single-line substitutions retain logical serialization positions; multiline
  actuals and substituted junction columns are refused. Embedded algorithm
  comment decoration preserves original spans; standalone parsing still requires EOF.
  Generated declarations precede trailing user definitions; unknown names still
  reject and invalid caller ranges carry bounded UTF-8 diagnostics.
- Bounded temporal operator actuals: `Len`, `Head`, and `Cardinality` retain
  scoped constant/state calls with independent word/witness checks and
  callback-free stored-input admission. Configured/opaque/effectful and
  unsupported state-storage/error boundaries remain explicit; final TLC
  scheduling remains separately scoped.

- Ordinary configured `RandomElement` safety checking uses actual bound native
  authority for predecessor streams and reports real inline worker use. Independent
  replay resolves constant targets and definition redirects; unproved startup,
  strict first-successor printing, transformed-key, Randomization replacement,
  graph/temporal and recovery paths retain explicit refusals (#180).

- Refuse unproved configured native random replacements before startup/output/search in
  both profiles and all capture routes, including source-visible/configured
  mixed calls. Preserve false-success and false-violation controls; direct RNG
  calls and pure user replacements remain supported (#180).

- Refuse configured VIEW with source-level random operators before binding/output
  or safety search in both profiles, including native aliases, cfg-selected
  native replacement targets and unused random definitions. Preserve the
  equal-count/different-draw defect; pure VIEW and untransformed random checking
  retain their supported implementations (#178).

- Normalize every concrete variable before evaluating VIEW, including ignored
  variables and runs without symmetry. Unproved TLC normalization failures
  explicitly refuse instead of reporting a model verdict with unprintable
  states (#188). Preserve concrete parallel/recovered witnesses.

- High-level PlusCal rewriting checks original source-option declarations before
  masking old translation blocks and refuses deciding declarations overlapping
  old output, preventing silent loss of requested fairness under either policy.


- Bounded PlusCal source fairness/label options now resolve requested and effective
  settings with original declaration spans for native generation and Full CRC.
  Unknown, malformed and conflicting contexts are refused; deciding declarations
  inside previous translation blocks cannot be silently masked.
- Bounded finite-process Full checksums now retain explicit fairness and
  automatic-label option context, with typed options in replacement reports.
  Unproved source option contexts remain named refusals; duplicate CLI fairness flags
  are rejected before loading. Pinned Validator context limitations are documented.


- Refuse unproved configured native random replacements before startup/output/search in
  both profiles and all capture routes, including source-visible/configured
  mixed calls. Preserve false-success and false-violation controls; direct RNG
  calls and pure user replacements remain supported (#180).

- Refuse configured VIEW with source-level random operators before binding/output
  or safety search in both profiles, including native aliases, cfg-selected
  native replacement targets and unused random definitions. Preserve the
  equal-count/different-draw defect; pure VIEW and untransformed random checking
  retain their supported implementations (#178).

- Preserve pinned PlusCal CASE assignment parentheses and empty-Else TRUE
  conjuncts, with complete generated-module syntax-tree regressions.


- Explicit final TLC mode now admits pure state conditional predicates through
  the corrected shared planner and fresh independent witness replay. Scoped
  LET/INSTANCE, entry/closing and error controls retain all earlier refusals;
  resource bounds and ordinary periodic scheduling restrictions stay unchanged.

- Explicit bounded final TLC checking supports proved pure user operator formals,
  retained LET/INSTANCE scopes and constant/state/temporal actuals. Independent
  periodic and serialized witness replay retain original attribution. Native,
  recursive/higher-order, configured-origin-ambiguous and effectful callbacks
  still refuse without raising resource limits. See `docs/temporal-operators.md`.

- Explicit bounded final TLC checking admits callback-free stored strings/model
  atoms, tuples, records, functions and Enum/Raw sets in captured scopes and
  configured constants. Actual context resolution discharges potential OPEN
  dependence; opaque/effectful/computed providers and fixed recursive admission
  limits still refuse. Supplied stored sets/records/general functions must match
  the evaluator ordering profile; tuple domains remain profile independent.
  See `docs/material-capture.md`.

- Explicit bounded final TLC results retain configured PROPERTY names, original
  expanded temporal Action names, and derived role labels separately in the
  Rust API and schema-2 JSON. Conjunctions preserve repeated original names;
  original witness claims, ordinary schema 1, scheduling and bounds are unchanged.
  See `docs/tlc-property-origins.md`.

- Scoped explicit final temporal checking preserves local LET derived role
  names separately from formal argument forwarding, and exact cardinality-one
  DNF shape. Caller contexts and original witness claims have pinned field and
  fresh replay controls. The later property-origin increment adds expanded Action
  names separately; ordinary schedules remain incomplete. See `docs/tlc-final-scopes.md`.

- Scoped explicit final temporal checking preserves local LET derived role
  names separately from formal argument forwarding, and exact cardinality-one
  DNF shape. Caller contexts and original witness claims have pinned field and
  fresh replay controls; expanded-action names and ordinary schedules remain
  incomplete. See `docs/tlc-final-scopes.md`.

- Explicit final/TLC checking admits proven direct temporal conditionals with
  pinned shared-guard tags, exact particles and independent counterexample replay.
  Compound guards and INSTANCE attribution have source-bound controls; forwarding,
  accepting-sink early witnesses, ordinary schedules and existing bounds remain
  explicit restrictions.

- Native safety checkpoint/recovery admits pure string/name construction, retaining
  the reported TLC inline encounter-order fallback and canonical parallel path.
  Runtime effects/checker reads and independent TLC runtime-name witness proof
  remain explicitly unsupported; queue/log budgets keep their separate scope.

- Pure safety native checkpoint/recovery now composes with opt-in frontier and
  predecessor disk budgets, including deterministic parallel VIEW/symmetry
  recovery and explicit finalized transient cleanup evidence.
- CI runs the complete workspace test suite in eight disjoint partitions, with
  unchanged test/job limits. The required workspace check fails if any
  prerequisite fails, is cancelled or is skipped.

- CI runs the complete workspace test suite in eight disjoint partitions, with

- CI runs the complete workspace test suite in four disjoint partitions, with
  unchanged test/job limits. The required workspace check fails if any
  prerequisite fails, is cancelled or is skipped.

- CI runs the complete workspace test suite in four disjoint partitions, with
  unchanged test/job limits. The required workspace check fails if any
  prerequisite fails, is cancelled or is skipped.

- Native pure safety checkpoints pause and resume through the deterministic worker
  pool, including supported VIEW/safety symmetry, cross-width recovery and
  ordered counter-limit stops. Boundary-zero pauses report one actual worker
  before pool startup; runtime/temporal/graph/spill restrictions remain explicit.

- TLC-shaped temporal planning preserves eager state-conditional conversion
  failures, typed TLC errors and source-order fallback before graph work.


- Native PlusCal translation and algorithm CRC derivation explicitly refuse unproved source `PlusCal options (...)` declarations before returning an artifact.

- Bounded finite-process Full checksum derivation for declared-fairness and
  source-owned modifier records, independently checked against reference AST
  bytes and admitted operator trees. Unproved option/name/call shapes remain
  named refusals. Runnable CLI/API examples separate CRC equality, artifact
  admission and model proof. Singleton identity clauses now follow the
  reference five-unit UTF-16 spelling threshold; original CST failure retained.



- Native bounded PlusCal Full checksum serialization now covers nested `elsif`,
  multiple `with` binders and LF/CRLF expression line grouping with normalized
  ASCII junction columns. C `with` trailing separators match the reference;
  empty/missing binders still reject. CRC derivation is separate from artifact
  admission and proof; multiline generation and empty-Then layout remain named
  refusals. Empty-Else TRUE fidelity is restored, with fair-only CRC weakness
  unchanged.

- Full reference AST/body CRC derivation for a proved bounded sequential PlusCal
  subset. Default rewrite requests expose exact serialized AST evidence; unproved
  shapes refuse by name. Fair-header CRCs retain the upstream weakness explicitly
  and all generation remains nonproof. See `docs/pluscal-rewriting.md`.

- Bounded in-memory PlusCal block replacement through `dive translate --rewrite`
  and Rust APIs. Explicit `--checksums tla` records body CRC and disables unproved
  algorithm checking; modified bodies need `--replace-modified`. Other AST shapes
  and file writes remain named unsupported. See `docs/pluscal-rewriting.md`.

- Native process-owned procedure fairness for bounded finite callers: reachable
  WF/SF clauses, process/procedure label modifiers and pinned option precedence,
  with formula/closure quotas. Scoped renaming and strict namespace witnesses
  remain separately pending. See `docs/pluscal-procedure-fairness.md`.

- Multiple disjoint finite PlusCal process blocks can call shared ordinary,
  recursive and self-tail procedures in both syntaxes. Owned slots/stack
  initialize over union ProcSet; procedure actions are generated once and
  global quotas span every body. Scoped rewriting/process fairness and strict
  namespace witnesses remain pending; invalid upstream artifacts are rejected.
  See `docs/pluscal-multiple-procedures.md`.

- Native PlusCal procedure calls from one finite process block: owned indexed
  parameter/local slots, stack/pc transitions, recursive and self-tail calls,
  both source syntaxes and explicit global limits. Algorithm-level fair syntax
  retains global WF_vars(Next) artifact generation; process-declared fairness
  and liveness execution are separate pending contracts. Process/scoped renaming and
  strict runtime-namespace witnesses remain pending; invalid reference artifacts
  are rejected. See `docs/pluscal-process-procedures.md`.



- Native recursive sequential PlusCal procedures: self/mutual call stacks,
  frame-preserving self-tail calls and source-bounded normalization. Both syntaxes
  retain pinned artifacts/errors without unrolling. Invalid upstream output is
  rejected; finite/count, strict pending and canonical witness evidence remain
  separate. Process-owned/scoped renaming and strict namespace provenance are
  still pending. See `docs/pluscal-recursive-procedures.md`.


- Native PlusCal translation supports multiple disjoint sequential procedures and
  nonrecursive nested calls, with procedure-owned frames and exact adjacent
  call/return normalization. Cycles, unproved name rewriting and process stacks
  remained named refusals in that earlier graph increment. Generated modules are validated artifacts, never proofs;
  strict namespace/deadlock replay gaps remain explicit.

- Native bounded sequential PlusCal procedure calls/returns: saved stack frames,
  parameter/local initialization, procedure action wrappers and original spans;
  unproved scopes refuse explicitly, with qualified replay evidence kept separate.

- Native PlusCal translation admits multiple disjoint finite process blocks, with
  source-owned scalar/indexed storage, shared CASE `pc` initialization, actions
  and fairness formulas. Aggregate bounds and named scope/domain refusals remain
  active; artifacts report no exhaustive search or model proof.

- Bounded single-process PlusCal translation supports weak/strong declarations,
  post-colon `+`/`-` label modifiers and pinned `-nof`/`-wf`/`-sf`/`-wfNext`
  option precedence. Full operator CST checks preserve fairness quantifier and
  junction grouping. API/CLI artifacts remain validated modules, never model
  proofs; multiple blocks, procedure fairness and unproved domain normalization
  remain named refusals (#155).

- Native PlusCal translation now admits one unfair constant-ID or finite-set
  process in current C/P syntax, matching scalar/indexed local storage, indexed
  program counters, process action scheduling and terminating stutter. Missing
  process labels require explicit `--label`, matching the pinned process default.
  JSON returns process source/storage metadata and local default-init `.cfg`
  bindings. Multiple blocks and unproved renaming remain named refusals;
  artifacts are validated modules, never model proofs (#150). Global `pc` is now
  rejected even when generated pc storage is omitted, matching its unconditional
  reserved symbol and correcting an inherited false translator acceptance.

- Temporal non-stuttering predicates compare their subscript before evaluating
  the action body, matching TLC liveness short-circuiting. Independent temporal
  trace evaluation uses the same order separately for angles, boxes and fairness
  occurrences; ordinary runtime action evaluation remains body-first.

- Rust safety-check requests reject a frontier directory without an encoded
  frontier budget before loading a model, matching CLI option admission.
- Bounded concrete temporal graph stage: retain original accepted successor values
  through the actual deterministic worker pool, commit one null-tableau OOS in
  TLC slot order and expose persisted bin/PEM graph inputs. All graph results
  remain explicit non-verdict evidence; wider tableau/schedule/lasso work remains
  refused. Scoped capture admission rejects opaque effects and malformed IDs
  before workers execute; fixed structural bounds are unchanged.

- Add bounded ordered temporal execution callbacks and actual multiple-OOS/
  tableau graph insertion, including immediate done-state re-enumeration.
  Retain deciding source-prefix errors separately from speculative pool work,
  typed failures and written graph evidence. Exact TLC SCC/lasso selection,
  accepting-sink witnesses and periodic schedules remain named pending work;
  ordinary strict liveness and resource defaults do not change.

- Bounded concrete temporal graph stage: retain original accepted successor values
  through the actual deterministic worker pool, commit one null-tableau OOS in
  TLC slot order and expose persisted bin/PEM graph inputs. All graph results
  remain explicit non-verdict evidence; wider tableau/schedule/lasso work remains
  refused. Scoped capture admission rejects opaque effects and malformed IDs
  before workers execute; fixed structural bounds are unchanged.

- Expose bounded pinned final checking through `dive check --compat=tlc
  --lncheck final --lasso tlc` and `TemporalMode::FinalTlc`, with private validated
  outcomes, schema-2 actual counts/stages/workers, full property identities and
  stuttering/lasso endings. Add a distinct `dive-final-tlc` harness contract with
  fresh independent replay and final-schedule golden admission. Resolve config
  replacement aliases before callback-free startup admission. Ordinary periodic
  checking, exact temporal proof and unsupported classes remain refused.

- Add explicit bounded `dive_api::tlc_temporal` final-only TLC fingerprint checking
  with a user guide in `docs/tlc-final-api.md` and callback-free pre-bind admission:
  preserve pinned SCC anchors/pop order, greedy witness choice and full configured
  property attribution, with fresh independent in-memory and JSON replay before
  a private violation certificate. All30 selected controls/120 width outcomes and
  original failures/source histories are retained. Exact-store proof, accepting-
  sink/initial replay, periodic schedules and ordinary strict CLI integration
  remain unsupported; fixed limits/defaults stay unchanged.

- Add bounded ordered temporal execution callbacks and actual multiple-OOS/
  tableau graph insertion, including immediate done-state re-enumeration.
  Retain deciding source-prefix errors separately from speculative pool work,
  typed failures and written graph evidence. Exact TLC SCC/lasso selection,
  accepting-sink witnesses and periodic schedules remain named pending work;
  ordinary strict liveness and resource defaults do not change.

- Bounded concrete temporal graph stage: retain original accepted successor values
  through the actual deterministic worker pool, commit one null-tableau OOS in
  TLC slot order and expose persisted bin/PEM graph inputs. All graph results
  remain explicit non-verdict evidence; wider tableau/schedule/lasso work remains
  refused. Scoped capture admission rejects opaque effects and malformed IDs
  before workers execute; fixed structural bounds are unchanged.

- Add bounded diagnostic concrete phase-L receipts: TLC successor slots, first
  representatives, logical capacity history and ordered null-tableau bins with
  partial error/work evidence. Initial particle consistency is separate. Wider
  diagnostic requests, tableau graph insertion, state-IF planning and opaque
  values remain named refusals; no checking defaults or strict temporal claims change.

- Add bounded diagnostic TLC binary particle tableaux: exact first-encounter
  node/edge order, state predicates, initial particles and promise identities.
  Complete pinned projections and independent periodic/closing-witness controls
  preserve explicit resource, structural and scope refusals. Strict integrated
  liveness, checker behavior and defaults remain unchanged.
- Pure finite safety checking can pause at a completed BFS level and recover
  the actual saved concrete frontier with `--checkpoint` / `--recover`.
  Native archives validate model/config/profile/executable identity, ordered
  names, keys and predecessors before recovery. Pauses are explicitly incomplete;
  malformed or interrupted archives refuse. Both profiles preserve TLC's counter
  reset by default; `--checkpoint-counters retain` preserves original totals.
  The first slice runs inline with one actual worker and refuses temporal,
  runtime-effect and transient-spill interactions. Pure supported VIEW/safety
  symmetry recovery recomputes configured keys while retaining original concrete
  witness states; equal keys do not certify a trace (ADRs 0033/0038).

- Ordinary safety BFS can spill predecessor/ordinal records with a separate
  `--predecessor-memory-bytes` budget and optional `--predecessor-directory`.
  Zero spills immediately; both stores and profiles preserve concrete replay.
  Corruption, I/O and cleanup failures prevent certification while retaining
  partial counts and worker evidence. This is transient storage, not a total
  RAM budget or checkpoint mode. Other checking modes refuse the setting.

- Public evaluator construction now returns typed admission errors before
  literal interning or callbacks. Scope/action creation checks IDs, arity,
  ownership and stored captures; independent trace and temporal endpoints reject
  foreign model/evaluator metadata before replay setup. Unpublished Rust callers
  must handle the new fallible constructor/scope results (ADR 0028).

- TLC-order simulation refuses runtime-name history that cannot yet be
  independently validated; canonical default sampling remains supported.

- Safety BFS can spill ordered concrete frontiers with `--frontier-memory-bytes`
  and `--frontier-directory`, preserving counts, effects and counterexamples.
  JSON reports frontier-only storage and explicit I/O/cleanup failures. Visited
  states, candidates, traces and temporal graphs remain outside this budget.

- Sequential PlusCal automatic labels match the pinned default and `-label` /
  `-labelRoot` options. JSON reports inserted label positions and required
  `defaultInitValue` configuration for omitted global initializers. Sequential
  `L:+` / `L:-` modifiers retain the program counter without inventing process
  fairness. All admitted artifacts pass enclosing-module checks; branch-internal
  labelled expansion and other unproved families remain named refusals (#143).


- Native bounded sequential PlusCal normalization/generation for explicitly
  labelled current C/P algorithms, with atomic sequential/parallel assignment
  semantics and stage-separated errors. `dive translate`, `translate_file` and
  `translate_source` return in-memory enclosing modules only after syntax/name/
  level admission; translation never claims model proof. Unsupported naming,
  processes/procedures/macros, expression scopes/layout, historical flags and
  rewrite/checksum/map-format remain explicit. Complete pinned evidence retains
  invalid reference artifacts, compares operator structure and validates finite
  generated models and corrupted witness controls (issue #131).

- Preserve TLC runtime string encounter order by using reported inline checking
  for reachable string/name constructors. Eagerly intern checker configuration
  values before assumptions, including the seed and fingerprint (issue #122).
- Independent trace validation explicitly refuses TLC runtime-name history
  that a witness prefix cannot establish, preventing a forged trace from
  choosing its own string order (issue #124). Canonical validation remains
  decisive; supplied seed/fingerprint metadata does not bypass this refusal.
  Caller-supplied temporal/enabledness predicates receive the same scoped
  checks; structurally proven tuple-only sequence calls remain supported.

- Offline parity-ledger checks reject absent or escaping evidence, changed
  stable IDs, unsupported certification claims and Markdown census drift.
- Native bounded PlusCal source parsing for current C/P syntax, with typed
  declarations, processes, macros, procedures and control flow; original byte
  spans, Unicode/tab display positions and stable resource diagnostics. Pinned
  grammar evidence accounts for 222 inputs/markers and retains normalization
  differences/timeouts. Translation/options remain unsupported; no runtime Java.

- Pure final/short temporal graph capture uses ordered parallel source waves,
  retaining all duplicate/seen edges, original labels/ordinals, roots and
  implicit stutters. Inline/1/4/8/chaos controls cover both profiles and stores.
  Existing logical graph limits are unchanged; a separately named per-source
  scratch admission bound returns an explicit resource refusal. VIEW, symmetry,
  randomness, stateful graph effects and strict TLC temporal timing still refuse.

- Native bounded `dive dfid` and `dfid_file` implement pure safety iterative
  visitation with TLC's discovery/status bits, eager vector counting and actual
  worker selection seeds. Depth-bound stops never claim model safety, and every
  claimed violation is independently certified. Remaining mode/output contracts
  refuse by name; TLC's stale cross-root witness is retained as reference evidence.

- Add a bounded diagnostic TLC planning projection: predicate occurrence tags,
  negation/simplification/DNF stages, ordered EA/AE and OOS/PEM grouping, and
  original configured property identities. Field-level pinned Java controls and
  independent scoped-word checks cover fairness and retained arguments. Strict
  integrated liveness/tableau identities remain unsupported; existing limits,
  checking behavior and defaults are unchanged.

- Retain pure temporal expression arguments through local and nested substituted
  caller scopes on the final/short path, including supported ENABLED formulas.
  Fresh independent periodic/whole-trace controls cover closing edges and forged
  witnesses. Exported INSTANCE temporal formals and recursive/higher-order
  domains remain named refusals; no limits or defaults change.

- Final/short temporal bindings admit closed pure captured filters over bounded
  finite structural inputs, and maps over those filters. Input work is bounded
  before predicate evaluation, cached filters retain their original upper bound,
  and caller state/priming is preserved. Six clean pinned controls match final
  truth/counts in both profiles/stores with independent periodic validation.
  Opaque, effectful, infinite and recursive scoped domains remain refused.

- Final/short temporal failures retain their execution stage, actual capture
  workers, completed statistics, output and collision qualifications. Unknown
  thread execution is reported as unknown rather than zero. Existing library
  error interfaces remain available alongside evidence-preserving entry points.

- Auditable TLC parity ledger and delivery plan covering native checking modes,
  distribution/recovery and PlusCal, with pinned source-surface inventories
  and separate accounting for strict and mixed-profile evidence.
- Conformance harness resolves relative executable paths before model/parse
  subprocesses change directory, preserving repository-relative provenance.

- Final/short temporal quantifiers and finite predicate constructors accept
  bounded structural function, record, product and subset domains. Preflight
  bounds intermediate materialization before enumeration and preserves TLC's
  operand normalization order; deferred and infinite domains remain explicit
  refusals. Eight pinned models match final truth/counts in both profiles and
  stores, with independently validated violations.

- Pure safety `SYMMETRY` now prepares bounded groups and canonicalizes dedup keys
  in both stores and the sequential/parallel engines. Concrete explored states
  and predecessors are retained for independent counterexample validation;
  `VIEW` applies after canonicalization. TLC mode preserves malformed-map
  behavior; the default rejects non-bijective generators. The required CM1
  gate adds the existing symmetry-bearing Paxos Voting model at 1/4/8 workers.
- Symmetry startup warnings retain original source tokens and configuration
  aliases (IR schema 5). Unverified startup exceptions, unsupported declaration
  shapes, deferred/lazy value permutation, randomness under symmetry and
  symmetry with liveness still refuse explicitly.
- `dive simulate` and Rust `simulate_file` sample finite pure safety traces, with
  TLC-exact seeded single-worker selection and default trace-identity replay.
  Every violation is independently validated; sampling never claims a proof.
  Unimplemented simulation modes and effects remain named refusals.

- Expanded non-gating final/short temporal evidence accounts for all 53 selected
  Examples models: 26 truth/count matches, 22 named refusals and five timeouts,
  with pins and reproduction recorded separately from strict conformance.

- A staged symmetry declaration API preserves pre-constant definition lookup,
  configuration substitutions and normalized function records. Early name/type
  errors match pinned TLC transcripts; effectful declarations refuse by name.
  Full binding, warnings and state reduction remain unsupported.

- A bounded symmetry-closure kernel preserves TLC generator/composition order,
  with checked bijections and TLC-compatible malformed maps. Binding and state
  reduction remain explicitly unsupported.

- Final/short liveness factors pure recurrence conditions out of temporal
  memory, reuses shared tableau expansions and retains only reachable product
  pairs, with unchanged resource defaults and deterministic witness ordering.

- Final/short temporal predicates retain action-level arguments to pure value
  helpers, including nested, captured and sequence-prefix calls.

- Final/short temporal predicates support bounded CHOOSE with retained domain
  levels, including pure Min/Max-style calls and state-dependent arguments.

- Final/short temporal predicates evaluate bounded state-dependent quantifier,
  set and function domains in each actual state/edge, with independent periodic
  validation and explicit refusals for infinite/oversized domains.

- Independent ENABLED validation preserves finite set/filter/function bindings, including current-state domains; primed domains and missing successor covers remain explicit refusals.
- Deep frontend campaigns use pinned SANY full checks and independent root-grammar observations, with deterministic mutations, complete case accounting and isolated copies.
- Independent disabledness validation refuses successor-dependent primed
  quantifier domains instead of binding them from the current state.

- Pinned ValidationTestSuite smoke corpus: 16 models and 20 modules, with a
  complete selected-case census, generated oracle goldens and a required
  1/4/8-worker conformance check.
- Final temporal predicates preserve finite constant-domain set-map, set-filter
  and function-constructor scopes without freezing captured state.

- `dive-conformance study final`: separate, non-gating final/short truth/count
  observations with fresh independent periodic-witness validation and explicit
  profile, binary, oracle and corpus provenance.
- Final temporal predicates retain pure INSTANCE value and variable bindings,
  including nested substitutions and closing-edge priming.

- Final temporal predicates and independent finite successor covers retain
  LET value and operator scopes, including captured state and nested calls.
- Empty symbolic function ranges and domains enumerate without expanding an
  infinite operand, preserving finite size, equality, hashing and fingerprints.

- Value printing breaks recursive diagnostic expansion of a non-enumerable
  function set, retaining a named error and symbolic fallback on small stacks.

- Bounded final temporal checking supports finite constant-domain state/action
  predicates; independent ENABLED covers certify finite forall/exists actions,
  including tuple bindings and empty-domain identities.


### Fixed
- Independent successor-existence proofs retain value-bound action-call scopes,
  allowing finite ENABLED/fairness decisions without sharing the checker
  enumerator. Missing covers and lazy state arguments still refuse.

- Final temporal operators retain pure state arguments lazily through caller
  contexts. Instantiated levels, nested forwarding, guards and priming are
  checked against independent periodic-word semantics.
- Final temporal planning and independent word validation inspect each scoped
  call separately; pure constant scalar/tuple arguments now bind through nested
  and quantified temporal operators. State-dependent and effectful arguments
  remain named refusals.
- TLC finite-prefix temporal violations are classified from their deciding
  message and complete finite trace, preserving the observed safety exit 12.
  Cyclic violations still require the ordinary temporal exit mapping.
- Successor traversal borrows the evaluator core instead of incrementing and
  decrementing its shared reference count at each recursive rule.
- Module-only conformance runs honor `--sany-only`; nightly parse checks no
  longer repeat model checking under the shorter parsing timeout.
- Checker error classification now follows TLC’s initial-state, successor,
  replay and postcondition paths. A pinned 175-model error matrix checks
  message codes and texts, exit status, counts and counterexamples in both
  stores at 1 and 4 workers, with explicit exclusions for unverified classes
  and ALIAS formatting. Postconditions see final counters; primed-variable
  errors retain the operand’s source location (IR schema 4).
- Seeded trace replay requires a proof that predicate and generator draw orders
  agree. It refuses unknown positions, including wide random expansions and
  coincidental matches, and checks redraw controls without a bounded search.
- Trace validator (issue #52): counterexamples whose values come from `RandomElement`,
  `RandomSubset` or `RandomSetOfSubsets` can be validated. Given the checking run's seed
  and polynomial (`TraceOptions::run`, `dive validate-trace --seed S [--fp N]`), the
  validator replays the run's draws with `dive-stdlib`'s `RunHost`, built without the
  successor enumerator: a fresh start-up stream for `Init`, and for each step the stream
  seeded with TLC's fingerprint of the state it leaves. Draws it cannot place (outside
  `Init` and the next-state relation, under `VIEW`/`SYMMETRY`, or in an order only the
  successor generator knows) are refused by name (ADR 0016, amendment 2). New `redraw`
  negative controls change a drawn value. Probe `P9Random` now agrees under
  `--compat=tlc` instead of `agree-pending-trace-validation`.
- Conformance harness (issue #55): under `--impl-profile tlc`, a worker-introspecting
  model (one that reads `TLCGet("config")`) run at N > 1 workers is compared with TLC's own
  result at N workers, on verdict and error class only, from worker-specific goldens
  (`<model>.workers-N.json`, `golden generate --workers N --reason ...`), which never
  replace the single-worker goldens. The default profile is unchanged (ADR 0004
  amendment). `MCDieHardest` at 1, 3, 4 and 8 workers now agrees in both profiles.
- Conformance harness (issue #48): dive's default profile now exits clean on the small
  tier. The D-010 differences in Examples `MissionariesAndCannibals` and
  `acp/ACP_NB_WRONG_TLC` (counts at the violation only) are registered in
  `conformance/expected-divergences.toml` and reported as `expected-divergence`.
  Registry entries now list the `fields` they may excuse, and never excuse trace
  validation. The nightly conformance workflow also runs the default profile on the
  tier's models.
- Conformance harness (issue #46): concurrent harness processes can share one
  `DIVE_CACHE_DIR`. Downloads, JRE unpacking, checkouts and jar module extraction run
  under a per-entry advisory file lock (`locks/` in the cache) and re-check under it.
  The README and `AGENTS.md` document a per-worktree `DIVE_CACHE_DIR` for parallel agents.
- `dive-sema` (issue #30): `DIVE-E0005` (a module defined twice, SANY 4223) is documented
  as a semantic-phase error, as SANY reports it, not a parse-phase one. The harness
  already classified it as `semantic-error`.
- `dive-store` and `dive check` (issue #42): the default profile's two-level state
  fingerprint combined per-value FP64 fingerprints with FP64 again, which is affine
  over GF(2), so the same change made to two variables could cancel; Examples
  `MCWriteThroughCache` lost 64 of its 5,196 states. The value fingerprints are now
  combined by a non-linear, position-keyed SplitMix64 chain (`dive_store::state::combine`,
  ADR 0015 §5 amendment), and `dive check` uses it again in the default profile, keeping
  TLC's own fingerprints under `--compat=tlc`. The JSON report's `collision` block names
  the scheme. Regression tests: `MCWriteThroughCache` in the default profile
  (`crates/dive-check/tests/fingerprints.rs`) and collision properties
  (`crates/dive-store/tests/state_fingerprint.rs`).
- `dive-eval`: an error inside `TLCEval` (including its argument) is reported wrapped in
  `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE`, as TLC's `EvaluatingValue` reports it.
- Conformance harness (issue #35): goldens and `dev capture` no longer record TLC as
  perturbed by `-dumpTrace json`, which loads `Integers` and its Java overrides into
  every spec. The deciding run omits it; a counterexample's JSON document comes from a
  separate trace pass that must reproduce the clean counterexample exactly. Provenance
  gains `trace_command`. The flag audit is in the harness README. All 141 Examples TLC
  goldens were regenerated with byte-identical payloads.
- `dive-eval` and `dive-value`: under TLC's 32-bit integers, `\div` in a spec that loads
  `Naturals` but not `Integers` follows `Naturals.Divide` (`Int::div_naturals`):
  `(0 - 1) \div 2` is `0`. The recaptured fixtures `ErGeqNaturals`,
  `ErPlusNonIntNaturals` and `RvNatDiv` now agree with dive.

### Added
- Positioned fallible disk membership and insertion for invariant-error replay,
  with frozen generated counters, typed source/ordinal failure evidence and
  original-error retention. Public disk role/mode restrictions remain (ADR0072).
- Native bounded C/P PlusCal macro expansion, simultaneous argument/assignment-selector substitution, nested prior declarations, original macro call/definition locations, and finite process/fairness interactions. Enclosing-module admission rejects invalid reference outputs; expansion budgets return named resource results. Full procedure/name/options/rewrite/map parity remains pending.

- Opt-in `dive check --lncheck final --lasso short`, using the bounded final
  temporal backend and independently validated lossless lassos. Version 2
  reports retain complete property claims, seed, store and actual worker width;
  TLC value profiles are labeled `tlc+overrides`. Ordinary checks stay on
  schema 1 and retain strict temporal refusals pending exact integration.
- Pure state- and temporal-guarded temporal IF formulas in bounded final
  checking and independent periodic-word validation, with named refusals
  for the oracle-rejected action guards and temporal CASE constructors.
- Final temporal planning and independent word validation support finite
  constant-domain quantifiers, tuple bindings and inspected pure configured
  constants. Original PROPERTY formulas preserve complete claim attribution
  when TLC splits quantified roles. Unbounded/effectful/dynamic cases refuse.
- `dive-api::temporal::check_final`: bounded final checking for the supported
  pure temporal subset, with independently validated lossless counterexamples,
  complete property claims and retained safety-stop evidence. TLC-exact lasso
  selection and ordinary `dive check` integration remain pending.

- WF/SF and direct ENABLED planning via scoped checker-side atoms. Bounded
  final temporal candidates now carry their complete false-property set,
  attributed by direct word truth and checked independently. Ten pinned
  violations and three successes match TLC counts in both stores; CLI temporal
  verdicts and TLC-exact planning/selection remain pending.
- Independent finite-cover successor-existence API with explicit domain,
  assignment and expression bounds; missing covers refuse by name. Supported
  deadlock claims certify independently. Direct ENABLED and WF/SF on periodic
  words decide finite enabledness; ten pinned liveness words validate.
- Bounded behavior-graph capture through the shared inline safety engine,
  retaining duplicate/seen transitions and explicit implicit stutters without
  changing counters. Incomplete searches never expose complete graphs.
- Final-only pure liveness library coordination: all behavior conjuncts and
  properties are planned before exploration; candidates retain concrete
  closing edges and target-property metadata. Fairness, TLC-exact selection,
  complete attribution and CLI integration remain pending. Four pinned pure
  models match TLC counts in both stores and independently validate.
- Independent fairness certificates for supplied periodic counterexamples:
  a recurring non-stuttering action establishes both weak and strong fairness.
  Prefix-only occurrences, unchanged subscripts and false actions cannot
  certify fairness. Other cases still require independent enabledness checking
  and refuse explicitly; this does not enable liveness model checking.
- Bounded behavior-graph/tableau products with separate state and per-edge
  action labels, promise acceptance and witness projection retaining original
  edge positions. Invalid inputs and resource limits are explicit errors.
  Model graph capture, TLC-exact ordering and checker integration remain pending.
- Independent validation of named pure liveness counterexamples: the optional
  trace `properties` list names every violated temporal property. Init, all
  edges including loop closure, constraints, safety and behavior predicates,
  and property truth are checked separately from graph search. Implicit
  stutters preserve TLC’s action-constraint semantics. Unestablished fairness, ENABLED,
  VIEW/symmetry and unnamed legacy liveness claims remain explicit refusals.
- Pure bound liveness predicate planning into the formula/tableau kernel,
  preserving state/action scopes. Unsupported constructs and resource limits
  carry source locations and stable diagnostic codes. Fairness/ENABLED,
  dynamic contexts, TLC-exact planning and checker integration remain pending.
- Bounded temporal formula/tableau kernel for boolean, always and eventually
  formulas over supplied state/action atoms. Negations normalize by duality;
  every eventuality has an acceptance promise. Malformed references and
  construction limits are errors. Full TLC-exact planning remains
  pending; the kernel alone does not produce model-checking verdicts.
- Independent temporal evaluation over supplied lassos in `dive-trace`:
  boolean combinations, `[]`, `<>`, `~>` and action atoms, including the
  closing edge. Fairness, `ENABLED`, temporal quantification and side effects
  remain explicit refusals. This is the temporal kernel for M6 L4;
  liveness checking and complete lasso validation are not implemented yet.
- Accepting-cycle search over supplied product graphs in `dive-liveness`: EA
  edge filtering, all required state/action visits and tableau promises, a
  shortest unrestricted prefix, and a deterministic short closed walk.
  Temporal planning and integration with model checking remain pending.
- `dive-liveness` graph foundation: validated dense references, iterative
  strongly connected components, and recurrent-edge filtering that preserves
  unrestricted prefix reachability. Formula planning and model verdicts
  remain pending.
- Approved M6 defaults and the explicit host-I/O grant policy are recorded in
  ADRs, the charter appendix, and D-005 / D-013–D-017. These options remain planned.
- M6 design: ADRs 0017 (TLC's liveness semantics), 0018 (liveness in dive) and 0019
  (symmetry), all proposed, and `docs/design/m6-plan.md`. 43 TLC probes for liveness,
  fairness and symmetry in `crates/dive-liveness/tests/fixtures/tlc-probes/`, with
  evidence that TLC's liveness counterexample is deterministic at one worker but
  depends on the fingerprint polynomial, and that symmetry with liveness both misses
  and invents violations.
- `dive-conformance dev capture --lncheck MODE --workers N --fp N`, for probes of TLC's
  other liveness modes. Goldens are unchanged.
- `dive-bench` (M5, WP-G1): a hand-run benchmark runner, not in CI. It times
  `dive check` at one worker in the `--compat=tlc` and default profiles on the set in
  `crates/dive-bench/benchmarks.toml` (`GameOfLife`, `TLCSailfish1`, `EWD998PCal`,
  `MCBakery`, `MCLamportMutex`, `SimpleRegular`, `MCbtree`, `CoffeeCan1000Beans`),
  checks every run's verdict and counts against the TLC golden, and reports median
  CPU time, wall time, distinct states per CPU second and peak RSS against TLC's
  golden, survey and (with `--tlc`) same-machine single-worker times.
- Standard library breadth (M5, WP-C3), each operator a port of the Java method TLC
  runs, with its errors:
  - the `Bags` overrides, and the CommunityModules operators that corpus models use:
    `SequencesExt` (`IsPrefix`, `FoldLeft`, `FoldRight`, `SetToSeq`, `SetToSeqs`,
    `LongestCommonPrefix`, `Contains`, `SelectInSeq` and friends, `RemoveFirst`,
    `Suffixes`, `AllSubSeqs`, `ReplaceFirstSubSeq`, `ReplaceAllSubSeqs`), `Functions`
    (`IsInjective`, `AntiFunction`, `FoldFunction`, `FoldFunctionOnSet`),
    `FiniteSetsExt` (`FoldSet`, `Quantify`), `Bitwise`, `BagsExt!FoldBag`, `Graphs` and
    `UndirectedGraphs` (`AreConnectedIn`, `SimplePath`, `IsStronglyConnected`,
    `ConnectedComponents`), `DyadicRationals`' `Reduce` and `IOUtils!atoi`;
  - `TLCGet` and `TLCSet`: registers (one logical register file), `"level"`,
    `"config"`, `"all"`, `"all:named"`, the counters while assumptions are checked, and
    `TLCSet("exit", TRUE)`;
  - `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` in `dive check`, drawing
    from the `--seed` streams (TLC's bit for bit under `--compat=tlc`; D-002);
  - `dive_eval::Host`, the interface through which the engine provides the level, the
    random stream, the counters and `TLCGet("config")`, and `dive_stdlib::host::RunHost`;
  - `dive validate-trace` and the harness's trace controls see `TLCGet("level")` and,
    when the run's `-seed`/`-fp` are known, `TLCGet("config")`.

  CommunityModules' `AllTests`: 651 of 767 assumptions agree (233 before), 116 refused
  by name, none wrong. Examples `DieHard/MCDieHardest`,
  `MisraReachability/MCReachabilityTestRandomGraphs` and `ewd998/EWD998PCal` now agree
  with TLC under `--compat=tlc`.
- Conformance (issue #18): the competitive-analysis probes (P1–P9, M1–M2) are the
  dive-owned corpus `probes` (`conformance/corpus/probes/`, Examples layout), with TLC
  and SANY goldens from the gating oracle. In-repository corpora are pinned by the
  SHA-256 of their files (ADR 0005, amendment 1). Under `--compat=tlc`, `dive check`
  agrees on 11 of the 14 models at 1 and 4 workers and refuses 2 by name (`Json!ToJson`,
  liveness); `P9Random` agrees on every compared field but is pending trace validation,
  because the validator has no random stream to replay `RandomElement`. The default profile's answers for P1 (unbounded
  integers, D-004) and P2's string case (canonical `CHOOSE`, D-006) are registered
  expected divergences. The nightly runs the corpus.
- Conformance: the competitors' measured invocations are external adapters in
  `conformance/adapters/`. The adapter schema gains provenance, library modes, a worker
  limit, environment templates, refusal exit codes, ordered output rules and count
  extractors.
- `dive check` claims TLC's class for a spec SANY would reject (`TLC_PARSING_FAILED`,
  3002, exit 150) in both profiles, verified against the harness fixtures and the M1/M2
  probe goldens; the harness compares it.
- `dive check --workers N` and `dive-check` (M5, WP-D3): the deterministic parallel
  engine of ADR 0015. Each level runs in three phases on a pool of workers that lives
  for the whole search: expansion with work stealing over blocks of states (narrow
  levels are split by action), a per-shard resolve in which the earliest position in
  TLC's order wins, and invariant checks on the new states. The search stops at the
  first event in TLC's order with ADR 0015 §3's closed-form counts, so verdict, counts,
  depth, counterexample, `Print` output and the JSON report (except timing and the
  `options.workers` block) are identical at every worker count, and equal TLC's
  single-worker results under `--compat=tlc`.
  - **Sequential mode**: specs whose checking path writes TLC registers, reads live
    counters or registers, draws random values, performs I/O, reads checker state, or
    has side effects inside lazily evaluated values are detected from the IR's effects
    (`dive_model::Model::sequential`) and checked inline with one logical worker; the
    output says so (`N worker(s) requested, 1 used` and the reason; JSON
    `options.workers.sequential` and `reason`). `--sequential` forces it.
  - **Ordered `Print`**: `dive_eval::capture` captures output per worker thread; the
    engine releases it in TLC's order, up to the stopping event. Once-per-run warnings
    are deduplicated at release, in order.
  - **Chaos mode** (hidden, test-only): `--chaos SEED` or `DIVE_CHAOS=SEED` randomizes
    block and chunk sizes, claim orders, the narrow-level split, inline phases, the
    resolve's shard order and thread count, and inserts yields.
  - `TLCGet("level")` is reported per worker thread while levels run in parallel
    (`dive_check`'s engine host wraps WP-C3's `RunHost`); randomness, registers,
    `TLCSet("exit")` and live counters keep `RunHost`'s single logical worker through
    sequential mode.
  - JSON `options.workers` gains `sequential` and `chaos`; `used` is the number of
    threads the search ran on (0 when the run ended before the search).
  - The required `Conformance gate` runs the CM1 list at 1, 4 and 8 workers
    (`scripts/conformance-gate.sh conformance/gates/cm1.txt --workers 1,4,8`; 17 s
    locally against 12 s at 1 and 4).
- `dive check` and `dive-check` (M5, WP-D2): the sequential checking engine, ADR 0015's
  level engine run inline with one logical worker, over `dive-model`'s bound model:
  - initial states, invariants (on new and out-of-model states), implied initial
    predicates, implied actions (on every step, including into seen states),
    `CONSTRAINT`, `ACTION_CONSTRAINT`, deadlock (no successor generated at all), `VIEW`,
    `ASSUME`s, and `POSTCONDITION`s after a complete search;
  - stops at the first event in TLC's order with ADR 0015 §3's counts; an error keeps the
    counts up to it; an initial-state stop reports no TLC statistics;
  - counterexamples rebuilt by replay from the trace log, with TLC's action labels;
  - fingerprint mode (TLC's own fingerprints under `--compat=tlc`) and exact mode
    (`--store exact`, D-007); TLC's collision figures on success;
  - refusals by name: temporal properties, `SYMMETRY`, whatever the binding stage or the
    evaluator refuses; an error whose TLC class is not reproduced yet is reported with
    the class marked unverified.

  `dive check Spec.tla [--config X.cfg] [--compat=tlc] [--store fp|exact] [--seed N]
  [--fp N] [--workers N|auto] [-deadlock] [--output human|json]`: TLC-style text or a
  versioned JSON report (verdict, statistics, counterexample as structured states,
  diagnostics, notes, and an `options` block with profile, options, polynomial and
  seed). TLC's exit codes under `--compat=tlc`, dive's own otherwise (`dive --help`).
  `--workers N` runs sequentially until the parallel engine (WP-D3) and says so.
  All 19 TLC probes and 11 harness fixtures agree with TLC in both stores, including
  every counterexample state and label (milestone CM0, `crates/dive-check/tests/probes.rs`).
- Conformance harness: `run --impl dive` checks models with `dive check` and maps its
  report to the oracle vocabulary. A violation whose only difference is the missing
  independent trace validation is `agree-pending-trace-validation`, never `agree`.
  `docs/conformance.md` keeps one section per implementation and profile (issue #27).
  `conformance/gates/cm1.txt` and `scripts/conformance-gate.sh`: the CM1 model list as a
  PR-gate candidate (not in CI yet; about 3 s per run). Small tier under
  `--compat=tlc`: 59 agree, 9 agree pending trace validation, 72 unsupported,
  0 disagree. Default profile, for information: 59, 7, 72, and 2 disagree, both at a
  violation under canonical exploration order (D-010).
- `dive-trace` (M5, WP-E1): the independent trace validator (ADR 0016 and its amendment 1).
  - A versioned counterexample format (`dive_trace::format`, version 1) with a lossless JSON
    form of values (`value_json`), which `dive check --output json` will embed; TLC's
    `-dumpTrace json` is read too, by searching the interpretations of its lossy values
    (`dive_trace::tlc`).
  - The checks: `Init` and the next-state relation as predicates over the given states,
    step labels, constraints, earlier invariants and action properties, the claimed
    invariant or property, assumptions and length. Results are `Valid`, `Invalid` (the
    first failing state and check) or `Unsupported` (named). Deadlock is refused by name
    after every other check, because deciding it needs the successor enumerator, which
    the validator is built without.
  - Negative controls: by-construction invalid mutants (`dive_trace::mutate`).
- `dive-api`: `trace` module, a small API for validating traces against a specification,
  including a Rust program's own implementation traces (`SpecModel`, `TraceDoc::behavior`).
- `dive validate-trace Spec.tla --trace trace.json [--config X.cfg] [--claim ENDING]
  [--compat=tlc] [--output json]`.
- Conformance harness: `trace_validated` is computed by the harness with `dive-trace`,
  in-process, never taken from dive; a deadlock is confirmed against the golden's own
  counterexample. New command `trace-controls` runs the positive and negative controls over
  the golden counterexamples: 12 of the 15 TLC safety counterexamples validate, 3 are refused
  by name (`TLCGet`, `Bitwise!Xor`, an `ALIAS` without the variables), none is invalid, and
  every mutant is rejected (D-012). Under `--compat=tlc` the small tier goes from 59 agree
  and 9 agree-pending-trace-validation to 68 agree; `agree-pending-trace-validation` now
  means the validator could not confirm the trace, and an invalid trace is a disagreement.
  `scripts/conformance-gate.sh` accepts only `agree`.
- `dive check --output json`: `counterexample.trace`, the counterexample in `dive-trace`'s
  lossless format, which `dive validate-trace` and the harness read.
- `dive-eval`: `Evaluator::redirected` and `Evaluator::is_value_override`.
- `dive-eval` (M5, WP-C2): the successor generator `dive_eval::next`, behind the
  `enumerate` feature (ADR 0014 §3 and its WP-C2 amendment):
  - ports of TLC's `getInitStates`, `getNextStates`, `processUnchanged`, `getActions` and
    `enabled`, rule by rule, generating initial states and successors in TLC's order with
    duplicates, including the all-assigned shortcut (`Options::warnings`), `x' \in S` in
    TLC's `elements()` order, lazy arguments that name a variable, lazy functions applied
    as actions, and TLC's error and warning texts;
  - `ENABLED`, the last C1 refusal (refused in a build without the feature);
  - the interface the checker drives: `Action`, `Scope`, `init_states`, `successors`,
    `next_states`, `Successor` (state, action, ordinal, completeness), `Flow`,
    `NextError`;
  - agreement with TLC on all 19 probes, on 18 new rule fixtures captured from the pinned
    TLC, and, through a test-only
    binding and single-worker search, on every small-tier Examples model it can bind:
    verdicts, counts, depth and counterexample lengths (`tests/next_examples.rs` commits
    22 of them).
- `dive-value`: `DeferredSet::as_any`, so the evaluator can enumerate its own lazy sets in
  TLC's unnormalized order.
- `dive-model` (M5, WP-B1): the binding stage, a port of TLC's `SpecProcessor`
  (ADR 0013 §8 and its WP-B1 amendment):
  - binds the configuration's constants (values, model values, `<-` substitutions,
    module-scoped `[M]` entries) and reports every configuration error with TLC's code
    and text, in TLC's order;
  - evaluates constant definitions at start-up as TLC does, module by module, with
    TLC's own level bound, so `Print` output appears when and as often as in TLC;
  - resolves `INIT`/`NEXT` or `SPECIFICATION` into initial predicates, the next-state
    relation, its actions (with TLC's labels) and fairness, and `PROPERTY` into
    invariants, implied initial predicates, implied actions and temporal properties;
    constraints, `VIEW`, `ALIAS`, `POSTCONDITION`, `SYMMETRY` and the deadlock flag;
  - fills the run's name table in TLC's encounter order: model values get TLC's exact
    intern tokens;
  - lists what the checker must refuse (liveness, `SYMMETRY`, operators dive does not
    implement), by name. Checked against TLC's own binding of 721 models: none wrong.
- `dive-eval`: `Scope`, TLC's context of a role, shared with the successor generator,
  and the evaluator methods that build and evaluate scopes; start-up values
  (`Evaluator::precompute`). The successor generator's test harness binds models with
  `dive-model` (one binding path). Uses of a constant or
  definition the configuration substitutes (`C <- D`) now evaluate `D`'s body each
  time, as TLC does.
- `dive-sema`: `Options::extra_root_extendees`, which loads modules as TLC's
  `-dumpTrace json` does, for oracle comparison.
- `dive-ir`: `Module::axioms`, `Module::tlc_constants`, `Ir::lexical_names`
  (`IR_SCHEMA` 3); a next-state relation that is a bare `TRUE` is one action named
  `TRUE`, as in TLC.
- `dive-eval` (M5, WP-C1): the interpreter over the IR (ADR 0014 and its amendment):
  - every expression form with TLC's evaluation order, laziness (thunks under
    `LazyValue`'s caching rule), lazy sets and lazy functions, `CHOOSE` in the run's
    value order, dynamic `INSTANCE` substitutions, recursion with a depth limit, and both
    integer modes;
  - TLC's error classes and message texts, including the wrapping of errors raised in
    Java overrides; anything not reproduced exactly is refused as `unsupported`;
  - the `Naturals`, `Integers`, `Sequences`, `FiniteSets`, `Strings` and basic `TLC`
    overrides, and TLC's assumption checking;
  - the pending owner questions as options with TLC's behaviour by default: exploration
    order (open question 9, D-010) and comparisons (open question 12, D-009);
  - 133 fixture specifications captured from the pinned TLC, plus `dive-value`'s 84,
    matched byte for byte; every `ASSUME` of the Examples corpus and the
    CommunityModules tests is either `TRUE` or refused by name.
- `dive-value`: a function remembers whether TLC holds it as a tuple literal
  (`Fcn::is_tuple_literal`), deferred sets can be lazy `\cup`, `\cap`, `\` and `UNION`,
  and `Set::as_deferred`, `components` and `raw_elements` expose what the evaluator needs.
- `dive-ir` (M5, WP-A1): the core IR and its lowering from `dive-sema` (ADR 0013):
  - every expression and operator form as an explicit variant, with SANY's level, an
    origin with SANY's line and column, and effects (`impure`, `live_tlcget`, `may_fail`,
    `uses_enabled`, `temporal`, `open`);
  - explicit laziness points (operator arguments, zero-arity `LET` definitions,
    substitutions), `INSTANCE` kept as dynamic substitutions, explicit recursion;
  - recognition of TLC's Java overrides (`StdOp`, extracted from the pinned jars) and a
    refusal, naming the operator, at every use of one dive does not implement;
  - deterministic ids, structural hashes, `serde` serialization (`IR_SCHEMA` 1), a
    debug dump and an invariant checker;
  - every module SANY accepts in Examples, the tlaplus tests and CommunityModules lowers
    (`crates/dive-ir/acceptance/README.md`).
- `dive-ir` action tree (M5, WP-A2): `actions::split` reproduces TLC's split of the
  next-state relation into actions (`Tool.getActions`), including its exception paths,
  configuration overrides and TLC's action labels; `actions::shape` gives the
  guards-and-updates view. On the 141 small-tier Examples models it reproduces TLC's own
  action list (names, parameters, labels, order and count) for every model.
- TLC's state-variable order (M5, WP-A3): `dive_sema::Model::variable_decls` and
  `constant_decls` reproduce SANY's `ModuleNode.getVariableDecls`/`getConstantDecls`,
  the reversed iteration order of the module context's Java `Hashtable`
  (`dive_sema::hashtable`). `dive_ir::Ir::tlc_var_order` records TLC's `varLoc` order
  (`IR_SCHEMA` 2). `dive_store::state::StateFingerprinter::tlc` computes TLC's own state
  fingerprint in that order, the scheme `--compat=tlc` uses; the default profile keeps
  dive's two-level fingerprint in IR order. The order matches the pinned TLC on 29
  generated specifications, 234 repository modules and all 424 Examples modules, and the
  fingerprints match TLC's `-dump dot` output on all 116 states of the generated
  specifications, from the specification text (ADR 0015 §5).
- `dive-model`, the configuration-binding stage (stub), and `dive-eval`'s `enumerate`
  feature (stub). `cargo xtask check-layering` rejects a standalone `dive-trace` build
  that enables the enumerator, and the workspace gate builds `dive-trace` on its own
  (ADRs 0013 and 0016, accepted).
- Cargo workspace with one crate per charter component, workspace lints,
  pinned toolchain, crate layering enforcement and lean CI.
- AI authorship protection: commit-msg hook, CI check and agent configuration.
- `dive-value` (ADR 0012):
  - 16-byte immutable, `Send + Sync` values with shared compound values and cached hashes;
  - arbitrary-precision integers (default) and TLC's 32-bit integers with its overflow
    errors (`IntMode`);
  - dive's canonical value order (default) and TLC's intern order (`ValueOrder`);
  - lazy sets (`a..b`, `SUBSET`, `[S -> T]`, `S \X T`, `[a : S]`, `Nat`, `Int`,
    `STRING`, `Seq`, evaluator-driven filters and maps) with membership without
    enumeration;
  - TLC's checked equality, comparison and membership, with TLC's error messages,
    distinguishing record literals from string-domain functions as TLC does;
  - TLC's value printer and pretty-printer, counterexample state format and JSON form;
  - TLC's FP64 fingerprints behind a `Fingerprinter` trait, verified in all 64 bits;
  - iterative drop, equality, ordering and fingerprints, and streaming of lazy sets,
    so deep nesting and huge intervals are safe;
  - differential fixtures from the pinned TLC, the red-team review's adversarial
    tests, property tests and benchmarks.
- `dive-store` in memory (ADR 0015, M5 work package D1):
  - a sharded 64-bit fingerprint set and an exact store of canonical state
    encodings, both with a level-synchronous resolve in which the earliest position
    of each new state wins, giving identical results at any thread count and
    schedule, without `unsafe`;
  - dive's two-level FP64 state fingerprint over cached value fingerprints;
  - per-level frontiers of encoded states, split into blocks for workers;
  - a 12-byte-per-state trace log of parents and successor ordinals for
    counterexample replay;
  - TLC's two collision-probability estimates, reproducing every figure in the
    conformance goldens and the "actual" figure from TLC's own fingerprints;
  - property tests under randomized schedules, and benchmarks.
- `dive-value`: a lossless value codec and a canonical exact-store key
  (`codec`), cached value fingerprints (`TlcFingerprint::fingerprint_cached`) and
  `Interner::resolve`.
- `dive-syntax`:
  - a TLA+ lexer and parser with a lossless rowan CST, including the proof language;
  - parse acceptance matching the pinned SANY parser;
  - stable `DIVE-S` diagnostic codes and error recovery;
  - typed AST views (ADR 0009).
- `dive-sema` (M4): SANY-equivalent semantic analysis (ADR 0011):
  - module resolution for `EXTENDS`/`INSTANCE`, nested modules and `LOCAL`, with
    SANY's search order, the embedded standard modules and the root-file fallback;
  - name resolution, arity, instances and substitutions, labels, subexpression
    selectors, recursion, and the proof language's scoping;
  - level checking ported from SANY;
  - stable `DIVE-E`/`DIVE-W` codes, each cross-referencing SANY's `ErrorCode`;
  - a resolved semantic model for `dive-ir` (`docs/sema-model.md`);
  - acceptance identical to the pinned SANY on every corpus module, SANY's error
    corpus, 164 minimal cases and 12,441 mutants checked
    (`crates/dive-sema/acceptance/`).
- `dive analyze FILE [--output human|json] [--library-path DIR]... [--module-path DIR]...`,
  with a versioned JSON report (`schema_version` 1), through `dive_api::analyze_file`.
- `dive parse FILE [--output human|json] [--cst]`, with a versioned JSON report
  (`schema_version` 1), through `dive_api::parse_file`.
- `dive-config`: the TLC `.cfg` reader. It covers:
  - every keyword the pinned TLC knows, including `_PERIODIC`, `_RL_REWARD` and `_POSSIBLE`;
  - TLC's constant value grammar and `<-`, `[Module]` and `A!B` forms;
  - typed `ModelConfig` with spans;
  - stable `DIVE-C` diagnostics, each with TLC's `EC` code, exit status and line.

  It matches TLC's own reader on 886 corpus cfg files and 99 mutants.
- `dive parse [FILE.tla] --config FILE.cfg` reports cfg diagnostics, and adds a `config`
  object to the JSON report (`dive_api::parse_config_file`).
- `dive-conformance run --impl dive` now runs dive's front end against the SANY
  goldens:
  - `dive analyze` against full SANY (default), and `dive parse` against SANY's
    parse-phase verdict of each file (`--sany-phase parse`), derived from the same
    goldens;
  - the module search path is derived from `oracles.toml` like the oracle classpath;
  - models stay `unsupported` until `dive check` exists;
  - `run` takes several `--corpus` into one report, and the summary breaks outcomes
    down per corpus;
  - the report records the dive binary (SHA-256, version), its module path, the
    profile and options.

  Results on 2026-10-08: all 1,309 runnable modules agree in both phases (Examples
  424, tlaplus tests 807, CommunityModules 78).
- Corpora `tlaplus-tests` (tlaplus `test/**` and `test-model/**`, 808 modules) and
  `community-modules` (78 modules, including `.smoke/`) have committed SANY goldens.
  `modules-only` corpora take `include` globs and `[[corpus.exclude]]` entries with
  reasons; excluded modules are reported as `out-of-scope`. One module that crashes
  SANY is excluded.
- The nightly `conformance` workflow builds dive and runs it on the modules of all
  three corpora, in both phases.
- `dive-syntax`: `tokens::tokenize_spec`, SANY's `SPEC`-state lexer as a public API.
- `dive-conformance`: the differential harness. It covers:
  - oracle and corpus fetching with SHA-256 verification;
  - TLC `-tool` stream parsing and verdict classification, cross-checked against exit codes;
  - SANY parse goldens;
  - golden generation with provenance (ADR 0003);
  - runs against goldens, with worker-invariance and manifest drift checks;
  - JSON reports and `docs/conformance.md`;
  - the nightly `conformance.yml` workflow;
  - isolated per-run copies of each spec directory, with `-seed 0`, `-fp 0` and `-Xss` pinned;
  - a `resource-limit` class for JVM out-of-memory and stack-overflow runs;
  - strict comparison for non-oracle implementations at every worker count;
  - an `--impl-option` hook (ADR 0010), where result-changing options yield `divergence-mode`;
  - oracle roles (`gating`, `reference`): v1.7.4 is reference, used only when named and never gating;
  - `conformance/oracle-changes.toml`, with a `known-upstream-change` outcome for cross-oracle differences that registered upstream changes explain.
- `conformance/oracles.toml`:
  - per-oracle classpath (v1.7.4 runs without CommunityModules);
  - pinned TLAPS-library and Apalache module libraries.
- `dive-stdlib::random` (M5 work package C5; D-002, D-003):
  - `java.util.Random` ported bit for bit (`JavaRandom`), with TLC's
    `RandomGenerator.nextPrime` and `-aril`;
  - TLC's seeding (`Streams`, `RandomMode::Tlc`): one start-up stream for constant
    definitions, `ASSUME`s and `Init`, and one stream per expanded state seeded with
    `fingerprint XOR seed`; TLC's simulation worker seeds;
  - dive's replayable seeding (`RandomMode::Dive`): counter-based streams keyed by
    seed, state fingerprint or trace number, and position, identical at any worker
    count;
  - `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` over `dive-value`
    values, drawing exactly as TLC's Java overrides, with TLC's error messages;
  - `-seed` parsing as TLC's `Long.parseLong`, and a fresh reported seed;
  - TLC fixtures at six seeds; dive reproduces all 728 printed values, the error
    messages, and the `SpanTreeRandom` golden's state counts from `-seed 0`.
- `dive-value`: `Set::as_fcn_set`, `as_rcd_set` and `as_product`.
- `dive-conformance dev capture --seed N`, for fixtures with a seed other than 0.

### Fixed
- `dive-value`: Rust `==`, `Hash`, `Debug` and `cmp_with` on an unnormalized set
  whose elements TLC cannot compare no longer mark nested unnormalized sets as
  normalized (issue #32). These operations now run in a quiet mode that records no
  TLC-observable side effect, and an element list computed that way is not cached,
  so a later TLC operation still normalizes the nested sets as TLC does.

### Changed
- `dive-eval`: constant definitions that read a configured constant are computed once, as
  TLC precomputes them, when every `<-` binding is constant-level and pure. TLC warnings
  are printed once each, and not at all with warnings off.


- Bounded TLC temporal normalization now retains shared IF-guard object tags
  and exact last-visit tag holes, with numbered-particle/edge and independent
  periodic-word controls. This source-level repair does not yet lift the staged
  final temporal-IF refusal or certify ordinary periodic scheduling (#170).

- Bounded non-junction multiline sequential PlusCal generation with explicit layout/substitution refusals, independent artifact/CRC/model controls and executable CLI/Rust guide.



- Independent whole-word tuple replay follows fixed nested tuple aliases in
  `UNCHANGED`; conditional permutations remain refused.

- Preserve complete storage, VIEW, and checkpoint recovery matrices as smaller CI test cases; retain timeout evidence and verify their compiled inventory without extending test budgets.
