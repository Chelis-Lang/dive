# Changelog

Notable user-facing changes are recorded here. The first release has not been
published. Earlier pre-release development entries remain in git history.

## Unreleased

- Core is now the default engine of `dive check`, `dive simulate`,
  `dive dfid` and the `dive tlc` adapter, and of `dive_api::check_file`,
  `check_file_tlc_inputs`, `check_file_checkpoint`, `simulate_file` and
  `dfid_file` (`Engine::default()` is `Engine::Core`). What Core does not
  implement is a named refusal (exit 3), never a fallback. `--engine reference`
  (or `DIVE_ENGINE=reference`, or `Engine::Reference`) runs the reference engine
  explicitly until it is retired. Migration: callers that relied on the
  reference default pass `--engine reference` or call `check_file_engine` with
  `Engine::Reference`.
- `--engine auto` and `Engine::Auto` are removed: `--engine auto` is a usage
  error (exit 2) that names the replacement. What Auto ran on the reference
  engine is now a named Core refusal that points to `--engine reference`:
  source and TLC logical coverage, external inputs (`--allow-io`) and
  recovery of a Reference checkpoint archive (`DIVE-S0400`). Check reports no
  longer carry the Auto-only `engine_requested`/`engine_fallback` fields;
  simulation and DFID reports keep `engine_fallback`, now always `null`.
- Bounded final temporal checking (`--lncheck final --lasso short`), multi-worker
  liveness with run-time names and finite modes that reach unreplayed random or
  register effects are named Core refusals; see
  [compatibility gaps](docs/tlc-differences.md#compat-gaps).
- Core keeps its start-up evaluation of pure constant definitions when the
  model has temporal properties: the binder evaluates only the definitions
  the temporal properties and the specification's fairness reach, instead of
  binding the model again and evaluating every one. `LoopInvariance/MCBinarySearch`
  (`PROPERTY Termination`) takes about 2.5 s on Core instead of 5.6 s, the
  same as without the property.
- Core computes `SYMMETRY` canonical images faster: images of values inside
  a state that recur across states are remembered per worker, within 32 MiB
  per run, and a set variable's image is derived from its predecessor's.
  `SimplifiedFastPaxos/Paxos` drops from 3.7 s to about 1 s and
  `SimplifiedFastPaxos/FastPaxos` from its time budget to about 18 s (TLC:
  1.1 s and 22.7 s), with unchanged counts.
- Core applies a built-in value operator passed as an operator argument, such
  as KeyValueStore's `ReduceSet(\intersect, S, b)`: the set operators, `=`,
  `/=`, `\in`, `\notin`, `\subseteq`, the Boolean connectives (still
  short-circuiting), `SUBSET`, `UNION` and `DOMAIN`, as if written in place.
  Applied as an action, or passed to a Java-overridden fold (where TLC itself
  fails), a built-in is still refused.
- Fix: Core reported an internal stale-handle error when a function's domain
  held a value retaining a nested record view, such as KeyValueStore's
  `PermSeqs` over logged operations while a temporal property is checked.
  `KeyValueStore/MCKVsnap` now completes with the pinned TLC's unreduced
  counts (D-005).
- Core evaluates each fairness condition of the specification once per
  liveness analysis instead of once per configured property, reusing the
  completed truth of every `WF`/`SF` condition rather than only the first `WF`.
  `SDP_Verification/SDP_Attack_New_Solution_Spec/MC` (six properties, fifteen
  `WF` conditions) evaluates 206,091 native atoms instead of 1,087,181.
  Verdicts, counts and counterexamples are unchanged.
- Core compares strings and model values by a rank in canonical order that
  the string table maintains, instead of by their text, cutting about 6% of
  the instructions of `CheckpointCoordination/MCCheckpointCoordination`.
  The order is unchanged in both profiles.
- Core refuses the `Json` and `IOUtils` operators (`JsonSerialize`,
  `ToJsonObject`, `IOExec` and the rest) only when a run evaluates them, as
  it does `TLCExt!Trace`: a trace export in a disjunct reached only after a
  violation no longer refuses the model, so `ewd840/EWD840_json` and
  `ewd998/EWD998ChanID_export` check on Core. Core still performs no I/O.
  A run that fails or warns no longer refuses for `Print` when the search
  printed nothing.
- Core evaluates a pure `ASSUME`, and the constant definitions only it reads,
  when it compiles its plan, so the binder no longer evaluates them
  (issue #846); a false or failing `ASSUME` is still reported exactly as
  before. Building a canonical set of tuples is also cheaper for every
  engine. `MisraReachability/MCReachabilityTestAllGraphs` takes about 0.7 s
  on Core instead of 7 s.
- Core evaluates a recursive operator or recursive LET function applied under
  a prime, as a refinement mapping `INSTANCE ... WITH v <- Op(e)` does under
  `[][Next]_vars`, in the successor state. `glowingRaccoon/product` now checks
  on Core; `ewd998/EWD998ChanID` reaches its VIEW-with-liveness refusal. A
  prime or `ENABLED` inside such a definition is still refused.
- Fix: Core passed a recursive call that reads the next state (an action-level
  recursive definition, or a state-level one in an initial predicate) to an
  operator as a value shared between disjuncts that assign different next
  states, losing states. Such a call is now evaluated in each disjunct.
- Core evaluates a definition that an action reads primed, such as
  `totalOpOrder'` in `InnerSerial`, once per distinct value of the variables
  it reads instead of once per candidate of an enclosing `x' \in S`.
  `SpecifyingSystems/AdvancedExamples/MCInnerSerial` completes in about 3 s
  with TLC's counts instead of stopping at its time budget. The cache never
  builds a set or function the action would have used without building it,
  and the copies it keeps are bounded by the memo byte cap.
- Core enumerates `{E \in SUBSET T : \A e \in E : Q}` as the subsets of
  `{e \in T : Q}` when `Q` is pure and does not read `E`, so
  `SpanningTree/SpanTreeTest5Nodes` (2^32 candidate subsets) checks on Core
  with TLC's counts instead of refusing.
- Core checks `SYMMETRY` with genuine temporal properties under the default
  `--symmetry-liveness=unreduced` (D-005), as Reference does: the whole run is
  unreduced and the report says so. `=refuse` and `=tlc` still refuse.
- Both engines check temporal properties when the only `RandomElement` is in
  a constant-level definition fixed at start-up, from the reported seed
  (D-002), so `SpanningTree/SpanTreeRandom` no longer refuses; under
  `--compat=tlc` it gives TLC's counts. A violation of such a model still
  refuses, because the independent validator does not replay start-up draws.
  A definition that a configured `Op <- D` or `C <- D` targets is not fixed:
  each use evaluates its body, so its draws still refuse on both routes.
  Nor is a definition whose start-up value keeps a lazy set (`{x \in S : P}`
  or `{e : x \in S}` over an infinite or deferred domain): each membership
  test evaluates it again, so its draws refuse too.
- In the default profile, Core evaluates the pure constant definitions its
  safety roles use as values once, when it compiles its plan, instead of the
  reference binder, and reuses the results of repeated calls while it does.
  `ewd998/EWD998ChanID_shiviz`'s `RingOfNodes` search drops from 16 s to
  under 1 s. A definition Core does not evaluate within its limits (256 MiB
  of transients, its recursion depth) still gets the binder's value. Core's
  `CHOOSE` frees rejected candidates as it searches.
- Core provides `TLCGet("config")` and the start-up counters to the
  `ASSUME`s, and `TLCGet("config")` during the search, as Reference does, so
  `DieHard/MCDieHardest` checks on Core.
- `dive simulate` no longer refuses `ALIAS`: as in `dive check`, the traces
  show the states themselves and the report notes that `ALIAS` is not
  applied. An `ALIAS` naming no definition still refuses.
- Core refuses `TLCExt!Trace` and `TLCExt!CounterExample` only when a run
  evaluates them, not when a specification merely mentions them.
- In the default profile, a constant definition that reads the environment
  without `--allow-io=env` no longer refuses the model when checking never
  uses it and its evaluation only reads (so `ewd998/SmokeEWD998` reaches its
  next refusal).
- Core checks standard operators passed to higher-order operators
  (`FoldFunctionOnSet(+, 0, f, S)`) and `DyadicRationals!Reduce`, so
  `ewd998/EWD998Small` and `Huang/Huang` no longer refuse. An error inside
  such an operator, and `Reduce` operands outside 32 bits, still refuse.

- Fold closed constant sets of up to 65,536 loop iterations at compile time
  (was 4,096), so type sets such as MCNano's `SignedBlock \cup {NoBlock}` are
  built once rather than in every state. Results are unchanged.
- Core liveness reads a state row for a temporal atom only when the reference
  evaluator needs it, not before every native evaluation; the implicit stutter
  reuses its source row. AllocatorImplementation reads 15% fewer rows and
  CoffeeCan1000Beans half as many.
- Core liveness evaluates prepared temporal atoms natively on state rows that
  hold sets, model values or functions with any keys, when every value is
  materialized. SDP and MCFlashWithMutexLive no longer fall back to the
  reference evaluator for their prepared atoms. Results are unchanged.
- Core liveness admits a temporal atom for native evaluation by Core plan
  facts instead of the reference stability check, and no longer caps plan
  size. CHOOSE over sets, Cardinality, set filters, SUBSET, UNION, records and
  multi-clause EXCEPT now run natively; recursive functions, `^`, `ToString`
  and negated ENABLED atoms still use the reference evaluator. At one worker,
  AllocatorImplementation drops from 101 s to 23 s, MCFlashWithMutexLive from
  40 s to 3.7 s and SDP_Attack_New_Solution from 6.5 s to 2.8 s. Results are
  unchanged.
- Core liveness also evaluates temporal atoms that call recursive operators
  or recursive functions, read memoizable definitions or use `^` natively. A
  native atom demand may spend up to 2^20 evaluation steps (was 4,096) before
  it falls back to the reference evaluator, and an atom that runs out 8 times
  in a row stays with the reference evaluator for the rest of the analysis.
  AllocatorImplementation drops from 14.3 s to 9.9 s at one worker. Results
  are unchanged.
- Core liveness no longer converts every generated state to reference values
  when it admits it to the temporal graph. It converts only a state's new
  strings and the values whose conversion could fail, such as sets of records
  or of mixed kinds, and remembers values already admitted. Refusals and the
  order of runtime names are unchanged. MCEWD687a drops from 2.8 s to 2.0 s
  at one worker (TLC 3.4 s) and CoffeeCan1000Beans from 9.0 s to 7.2 s (TLC
  20.0 s). Results are unchanged.

- Independently validate pure bounded TLA-mode action composition, including
  actual closing edges, deadlock and fairness. Nonstuttering compares the
  original and final states; alternate modes and unproved cases still refuse
  (D-022).

- Prepare native Linux x86_64/aarch64 release archives, SHA-256 checksums and
  bundled-library license notices. No version bump, tag or publication is part
  of this preparation.
- Replace historical workstream narratives with the first-value release gates
  and a user-oriented README.

## 0.1.0

This is the curated draft for the first release, **not a published release**.
The [release gates](docs/RELEASE.md) must pass before it is tagged.

### Added

- Native TLA+ parsing and semantic analysis, supported PlusCal translation,
  and explicit-state checking without a JVM.
- Safety and supported temporal checking, including fairness, with clear
  refusals for unimplemented features and explicit run-budget stops.
- Counterexamples independently validated against their specifications.
- Core evaluates pure bounded action composition with complete enumerable
  intermediates and deterministic tails assigning every final variable.
  Duplicate witnesses count as final candidates; state-dependent expressions
  follow each intermediate source and unproved shapes keep named refusals.
- Schema-3 JSON operation reports, pinned by fixtures for seven Core checking
  outcomes, with one shared API version constant for validation errors.
- Per-command help without input files, source revision information in
  `--version`, and human checking headers naming engine and workers. The
  unimplemented `lsp` command is omitted from help.
- Embedded standard modules, TLAPS and CommunityModules, with local modules and
  explicit search paths taking precedence. Analysis JSON identifies bundled
  sources; ordinary imports need no extra library flags.
- Linux x86_64/aarch64 archives with checksums, both command entrypoints, and
  the bundled libraries' license and provenance notices.

### Changed

- Merge already canonical Core union operands in order, preserving complete
  identity/class checks, symbolic fallbacks and cooperative interruption.

- Core builds finite sets canonically, removing TLC construction-history and
  receiver-authority tracking. Default mixed structured sets have consistent
  mathematical identity; unproved symbolic identities still refuse (D-028).
- Generate admitted scalar function-space `CHOOSE` candidates incrementally
  during model startup, preserving choice order, errors and resource controls
  without retaining the complete function space.
- Core evaluates applied LET-recursive functions with lexical captures and pure
  recursive applications under prime. Separate callee frames preserve INSTANCE
  substitutions; escapes, unproved captures and effectful primed functions refuse.
- Applied recursive functions resolve enclosing pure operator callbacks, including
  lambdas and forwarded parameters, in their lexical context. Unreachable
  captures, effects and unproved recursive calls retain named refusals.
- Restore top-level mutually recursive operators used by Nano's balance
  calculations. Key recursive compilation by lexical activation and INSTANCE
  bindings, preserving holding and violated invariants; mutually recursive
  captured local definitions retain their named refusal.
- Effect-safe recursive Boolean action guards avoid duplicate predicate witnesses
  and unnecessary Boolean alternatives (D-029). Default differences are precisely
  registered; nominal TLC retains documented count and short-circuit gaps.
  Recursive calls and arguments stop safely at 64 activations and poll budgets.
- Native words after empty product/record Init branches validate through actual
  pure Init emissions. Forged first states reject; earlier errors, partial states
  and interrupted replay cannot prove membership. Partial-Init witnesses and
  existing trace versions keep their meanings.
- Core temporal reads reuse fully materialized finite-set conversions within
  the existing bounded cache. Compound membership reuses its materialized-element
  admission proof while retaining query and interruption checks. These changes
  establish no release speed result.
- Sequential `ENABLED` assignments and `UNCHANGED` checks avoid repeatedly
  copying unshared partial query states while preserving retained alternatives
  and lazy evaluation snapshots.
- Benchmark runs stop before a later arm after an invalid or incomplete sample,
  retain physical process output and report actual worker use. Core selection is
  explicit; incomplete runs cannot supply a complete-suite performance result.

- Resolve configured constants through INSTANCE. Keep failed startup register
  writes from becoming cached successes, including lazy field forces; preserve
  ordinary Print/RNG startup values.

### Compatibility and limits

- Reference remains the default checker; `--engine core` selects Core without
  fallback. First-release Core coverage and speed must meet the release gates.
  Unimplemented requests refuse by name; budget stops are not success.
- `--compat=tlc` retains inexpensive compatibility switches and documents its
  gaps; exact reproduction of TLC implementation artifacts is not promised.
- EWD998ChanID imports resolve, but checking may retain its specific named
  VIEW/liveness or recursive LET refusal. The bare-use smoke requires the other
  nine original Examples to check successfully; this gives no Core parity credit.
- Distributed workers, debugging, recovery breadth, coverage and LSP are outside
  the first release's acceptance scope. Existing bounded implementations do not
  imply complete support for those modes.
