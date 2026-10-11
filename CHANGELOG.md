# Changelog

All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased

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


