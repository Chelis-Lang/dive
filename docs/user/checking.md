# Safety checking

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers auto --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --output json
```

## TLC logical coverage

```sh
dive check docs/user/examples/Coverage.tla --coverage 0 --compat=tlc --seed 7 --fp 0 --output json
dive tlc -coverage 0 -workers 4 -seed 7 -fp 0 docs/user/examples/Coverage.tla
```

Both commands complete the two-variable example with three distinct states,
four generated states and depth three. Auto selects Reference before execution
and reports one actual worker. `--coverage 0` corresponds to TLC `-coverage 0`:
produce the final logical action-context report. Coverage defaults to off in
both profiles, matching TLC without `-coverage`. Nonzero reporting intervals
are explicitly unsupported. The default semantic profile remains available;
its existing value-order and integer defaults still apply. Use `--compat=tlc`
when comparing against TLC's value semantics.

The JSON `tlc_coverage` object contains separate Init and split Next action
trees, original source ranges, logical invocations and enumeration costs.
Repeated parameterizations of the same action predicate share a tree. Different
calls to a helper within one action retain their own paths. Source labels use
the original declaration header, including parameters; unvisited assignments
retain TLC's zero rows. These are logical costs, not native expression-entry
counts. Collection observes actual producer demands and concrete set identities;
rendering or snapshotting never evaluates a TLA+ expression.

Assignment source markers follow formal arguments and aliases, including a
helper that primes an unprimed variable actual. Markers are shared by source
predicate across the calling forest. Their non-evaluating scan stops at an
equality or membership predicate, preserving TLC's rows for unused branches
without demanding either side's values.

The admitted slice supports multiple scalar state variables, explicit INIT/NEXT,
acyclic first-order helpers, scalar arithmetic/boolean expressions, IF actions,
finite literal/interval domains, interval-based powersets and differences,
bounded quantifiers and CHOOSE. Concrete literal-domain enumeration includes
TLC's prefetch demand and early termination. Existing laziness shortcuts are
disabled only while measuring logical argument demand; unused effects remain
unused and repeated lazy uses retain the ordinary thunk cache.

Configured constants, SPECIFICATION/property/constraint roles, VIEW, symmetry,
INSTANCE, recursion, higher-order operators, functions/compound state values,
captured LET/substitution contexts, unproved set materialization, Core/ENABLED,
temporal, checkpoint/recovery and sampling combinations are outside this slice.
Unsupported source shapes refuse
through the ordinary unsupported model path. A reached cost operation without
an implemented map yields `tlc_coverage.status: "unsupported"`, with a named
`feature` and no action costs; the original model result is retained.

Failed, stopped or unsupported runs also refuse the coverage feature. The pinned
TLC adds diagnostic-reconstruction work after an error; dive does not execute
those effects again to manufacture matching costs. The original verdict,
statistics, output and independently validated error word remain separate.
Complete successful reports have `status: "available"`. Neither status supplies
a proof beyond the ordinary model report. Counter saturation and the bounded
calling-tree/set-source collector capacities refuse exact costs.

Rust callers set `CheckRequest.tlc_coverage = true`; the ordinary checking API
returns the typed `CheckReport::tlc_coverage()` snapshot. Its `unsupported` field
names a coverage refusal. Native `source_coverage` and logical `tlc_coverage`
requests are mutually exclusive. Low-level callers use `SourceCoverage::tlc`
and the existing observed binding entrypoints, which prepare the action forest
before generation. Full TLC coverage, variable-value summaries, compiled source
maps and debugging remain mandatory unfinished work (ADR 0150).

## Native distributed workers

For explicit authenticated pure Core workers on independently initialized
processes, see [distributed safety](distributed-safety.md). Repeat
`--distributed-worker IP:port` with `--engine core`, explicit seed/fingerprint
and a local `--worker-key FILE`. Omission preserves local checking in both
profiles. Remote source evaluation and ordered coordinator commit do not provide
source coverage or an aggregate memory/performance guarantee.

## Native source work

```sh
dive check docs/user/examples/Counter.tla --source-coverage --seed 7 --output json
dive check docs/user/examples/Counter.tla --source-coverage --compat=tlc --engine auto --workers 4 --seed 7 --fp 0
```

This option reports actual producer expression evaluations, Init/Next walker
visits and generator emissions at original source locations. The example succeeds
with five distinct states; Auto selects a whole Reference run before execution,
with one actual worker even when more were requested. The default is off, which
is the TLC-equivalent setting. Both semantic profiles can request these native
metrics explicitly; they are **not TLC's logical coverage costs**. Pinned TLC
`-coverage 0` has the separate supported logical-cost route described above.

JSON adds `source_coverage` with `metric: "native-source-work"`, exact file/module,
UTF-8 byte ranges and JavaCC line/column positions, static instance-definition
names and five named counters per source row. End byte offsets are exclusive;
end columns are inclusive and follow UTF-16/tab expansion. Rows with zero work remain available; embedded
standard-module rows are excluded and synthetic origins are separate. Human
output shows the visited source rows. Native counts can change with evaluation
and caching strategies, independently of model semantics.

The context is explicitly `static-ir-origin`: nested callers that reach the same
original instantiated definition share its row. For example, outer `A` and `B`
instances can both contribute to one inner `I!Double` source row. These rows do
not reconstruct a dynamic calling tree or TLC's action-context costs.

Observation begins before producer constant/ASSUME evaluation and includes the
actual safety producer's Init, Next and property checks. It stops at checker
return: independent witness/diagnostic replay and report presentation cannot
inflate the rows. A failure or budget stop reports the actual producer prefix,
with its original verdict/completion information. Generator emissions include
candidates later rejected by constraints or already seen; they are not a count of
distinct states or committed discoveries. Counters never supply a model proof.
Saturation sets `overflowed: true` instead of wrapping into smaller counts.

Rust callers set `CheckRequest.source_coverage = true` and use the existing
checking functions; `CheckReport::source_coverage()` returns a typed snapshot.
Low-level callers can create a `dive_eval::SourceCoverage`
for their exact `Arc<Ir>` and use the observed evaluator/binding entry points;
ordinary constructors and fresh replay recipes stay unobserved. Snapshotting
counts does not execute a specification. The file API returns stopped collection;
a manually shared live collector's snapshot is not coherent across concurrent
entries.

The first file route supports ordinary Reference safety BFS. Explicit Core,
explicit temporal mode, checkpoint/recovery and core-sample combinations refuse.
Genuine temporal properties still refuse rather than passing as safety. Models
containing `ENABLED` currently refuse this option because its compiled query body
has no source map; unobserved checking retains its existing support. Core source
maps, TLC logical costs and source debugging remain work in progress.

Invariant expressions that use `ENABLED` through an `INSTANCE` retain the
query context inside native callbacks such as `SelectSeq`. A callback that reads
a primed variable observes the query's candidate assignment, including when an
inner variable shares a name with an outer variable. This applies in both
semantic profiles.

Bounded temporal callback analysis can admit native sequence concatenation
(`\o`) when an owned source or stored value proves that at least one operand
cannot successfully produce a string. The proof follows lexical aliases and
requires every result arm of an `IF` or `CASE` to have that property. It does
not evaluate an operand, force a cached thunk or inspect state invariants.
Strings and unknown result kinds retain the runtime-name refusal.

This capability removes only Concat's own string-name creation concern. Both
operands and all retained captures, including unused actuals, still undergo the
existing effect, provider, context ownership and resource checks. Configured or
redirected native origins and higher-order native operator actuals retain their
existing restrictions. Ordinary evaluation preserves eager argument order and
typed errors. Eligible single-worker temporal requests can report an original
Concat evaluation error through the certified row-demand route described below;
unproved contexts retain `DIVE-L0033` refusals. VIEW graph capture
still requires quotient-edge and closing-state evidence (issue #428), so
EWD998ChanID remains unsupported. Other temporal restrictions also remain.

State substitutions through `INSTANCE` retain their caller's state when a
native arithmetic operator reads them. For example, an instantiated
`Double == 2 * v` with `v <- x` reads the current `x`, in both semantic
profiles. The reference engine previously misclassified this expression as
constant and raised an undefined-variable error (#515). This minimal example
now succeeds with 1 distinct state, 2 generated states, depth 1 and exit 0:

```sh
dive check conformance/corpus/reference-instance-naturals/specifications/RefSubstDouble/RefSubstDouble.tla --engine reference --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

Actual constant definitions still retain their startup values, including
deferred-set identity and retained materialization history when a constructor
uses its own bound values. Configuration redirects keep their existing
evaluation behavior; this correction does not change startup output, lazy
argument demands or error priority. The separate core cache correction (#634)
covers definitions reached through two nested instance scopes. These bounded
controls do not establish general INSTANCE completion.

## Experimental core engine

Try the same small specification through four ordinary checking outcomes:

```sh
dive check docs/user/examples/Counter.tla --engine core --seed 0 --fp 0 --workers 1 --output json
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --engine core --seed 0 --fp 0 --workers 1 --output json
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --engine core --seed 0 --fp 0 --workers 1 --output json
dive check docs/user/examples/Counter.tla --engine core --seed 0 --fp 0 --workers 1 --max-states 2 --output json
```

These return safety success, a `StrictBound` invariant violation, an
`EventuallyFour` temporal violation, and a structured state-limit stop,
respectively. Their exit statuses are 0, 12, 13 and 5. The temporal violation
is valid: the specification allows stuttering and declares no fairness.

Rust callers can run all four outcomes and independently replay both
counterexamples in a fresh source-bound validator:

```sh
cargo run -p dive-api --example checking_preview --locked
```

The example uses `check_file_engine`, `TraceDoc` and `SpecModel`; it runs the
checker and validator in process. See the [Rust API guide](rust-api.md) for
the supported library interfaces.

```sh
dive check Spec.tla --engine core --workers 4 --output json
dive check Spec.tla --engine auto
DIVE_ENGINE=core dive check Spec.tla --output json
```

`--engine core` (or `DIVE_ENGINE=core`) runs the check on the new execution
core (ADR 0117), which is under construction. The reference engine stays the
default (`--engine reference`), and it is also the TLC-equivalent setting. The
core currently covers:

- the default profile and `--compat=tlc` (below). Safety checks support
  `--store fp|exact`, ordered frontier spill and predecessor-log spill under
  their explicit budgets. Disk seen sets and `--chaos` remain unsupported.
  Temporal checks with exact stores or frontier/predecessor spill refuse by name;
- any number of workers. Every result (counts, verdict, trace, output) is
  the one-worker result;
- safety: invariants, constraints, `ACTION_CONSTRAINT`, deadlock, and
  the parts of a `PROPERTY` that are safety: `[]P`, its initial predicates
  and its actions `[][A]_v` (a refinement `PROPERTY` of another
  specification is of this kind), `SYMMETRY` (under `--compat=tlc`, with
  TLC's comparison; a comparison TLC could fail on is refused),
  `VIEW` (with `SYMMETRY` too, evaluated on the canonical state as TLC
  does), and `ALIAS` as the reference engine handles it (not applied yet,
  with a note). `POSTCONDITION` is reported as unsupported, naming the
  feature;
- liveness: a `PROPERTY` that needs liveness checking, with fairness. The
  core explores the states and builds the behavior graph; the reference
  engine's liveness checker then checks it, with the same schedule (in the
  default profile, at completed levels, D-014; under `--compat=tlc`, once
  the search is complete), the same independent validation of a
  counterexample, and the same report. A temporal property together with
  `VIEW`, `SYMMETRY`, `Print`, `RandomElement`, `TLCSet`, `TLCGet`,
  `--max-states` or `--max-depth` is reported as unsupported. So, for now,
  is a run whose liveness check fails rather than answers (a planning or
  evaluation error, or a graph limit): `--engine auto` reports the
  reference engine's result for it. Capture retains original accepted targets,
  including seen successors, with their action and ordinal. Parallel capture
  buffers at most eight source states. Ordinary core files use the actual run
  time/memory budgets with checked layouts and fallible growth across capture,
  products, acceptance and attributed words; inherited count ceilings no longer
  decide capacity. Explicit Rust limits and reference routes retain their existing
  bounds. Genuine budget stops preserve their stage, statistics and capture
  completion instead of becoming generic refusals; supported temporal combinations
  remain unchanged. Temporal expressions retain
  the original distinction between general functions and tuple/record literals,
  including nested values, independently of report formatting;
- verdicts, TLC's distinct and generated counts and depth, counterexample
  traces in the reference engine's format, TLC's evaluation-error endings
  where the core can reproduce them, and the run budgets (`--time-limit`,
  `--max-memory`, `--max-states`, `--max-depth`).
- `Print`, `PrintT` and `ToString`, printing what TLC prints, in TLC's
  order. A specification that prints runs with one worker (the header says
  so). A run that prints and then finds a violation or an error, a print
  inside a function constructor, filter or set map, and a set that TLC may
  hold as an interval and print as `1..3` (a set of consecutive integers,
  in a specification that uses `..` or `DOMAIN`) are reported as
  unsupported.

Core temporal analysis reuses successful closed-input checks for exact retained
rows within each analysis, in both profiles. Optional proof storage is limited
to one MiB; allocation failure or a larger graph retains the original checks.
Current run controls, input/evaluation limits, conversion demands and independent
counterexample validation still apply. Additive
`core.metrics.liveness.atoms.row_proofs` counters report actual reuse and storage;
they do not establish faster original-model completion. There is no new option.
See [ADR 0134](../decisions/0134-native-atom-row-proofs.md).

`ENABLED A` inside an action constraint or implied action property searches
for a successor of `A` from the current state. Its primed expressions read
that query's candidate, including definitions also used by the surrounding
property. This example checks both contexts and returns success with 3
distinct states, 4 generated states and depth 3:

```sh
dive check crates/dive-api/tests/fixtures/core-m0/EnabStepCache.tla --engine core --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

The core also evaluates pure, nonrecursive `ENABLED` queries through
`INSTANCE` constant and operator parameters when each instantiated variable maps to the root variable with the
same name. This includes nested instances. For example, the following returns
success with 4 distinct states, 5 generated states and depth 4 in both profiles:

```sh
dive check crates/dive-api/tests/fixtures/core-m0/EnabConstInstance.tla --engine core --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

Hidden or renamed instance variables, instance queries inside an initial
predicate, state-dependent configuration operator overrides, and query bodies
with effects or recursive/nested calls remain explicitly unsupported by the
core. `--engine auto` selects the reference engine for these cases. The same restrictions apply in both
compatibility profiles.

Cached core definitions also retain their own `INSTANCE` substitution context,
including nested instances and primed definitions in a step property. A shared
source definition can therefore read `x` through one instance and `x + 2`
through another. The core previously reused the first compiled body, causing
both false invariant violations and missed violations (#634). This example
now succeeds with 3 distinct states, 4 generated states, depth 3 and exit 0 in
both profiles:

```sh
dive check conformance/corpus/core-instance-scopes/specifications/ScopeHolding/ScopeHolding.tla --engine core --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

The adjacent `ScopeViolation` model fails its invariant immediately with exit
12. These controls preserve existing unsupported query contexts; they do not
claim complete INSTANCE support. The reference cache repair is a separate
change (#515).

When constraints exclude every initial candidate, the core completes with
zero distinct states and no counterexample. As in TLC, the reported depth is
one if Init generated any candidate, and zero if Init generated none.

If Init retains states but the model defines no next-state action, the core
reports TLC's model error 2115 (exit 75), without a counterexample or TLC
statistics. This error also occurs with deadlock checking disabled. A defined
action that generates no successors instead follows the deadlock setting;
an empty Init completes without either error.

Core evaluates LET-bound recursive functions used through `f[arg]`, retaining
the actual enclosing values across recursive and repeated calls, including
`INSTANCE` substitutions. An unused failing LET binding or argument stays
unevaluated. Escaping recursive function values, primed applications, enclosing
operator parameters and mutually nested recursion remain named refusals in both
profiles.

The core can evaluate `SelectSeq(sequence, Test)'` when `Test` is pure and
depends only on its element parameter and constants. The sequence reads the
successor state. This also works through `INSTANCE` refinement substitutions,
such as counting messages selected by `msg.type = "pl"`. A callback that reads
state, captures an enclosing local, reaches an effect or makes an unproved
recursive call remains unsupported under prime. Other higher-order operators
under prime retain their existing refusals. These restrictions apply in both
profiles; `--engine auto` retries the reference engine on a core refusal.

For a small refinement example, this command completes with 3 distinct states,
4 generated states, depth 3 and exit 0:

```sh
dive check crates/dive-api/tests/fixtures/core-primed-selectseq/PureInstance.tla --engine core --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

The original pinned `EWD998Chan` model also enters core search with its original
configuration and refinement property. A one-second, one-GiB probe stops with
exit 5; that probe establishes admission only, with no completed verdict or
performance/capacity claim. It needs the configured CommunityModules and
dependency module-library roots, as well as its adjacent `EWD998` module.

With `--engine core`, anything the core does not implement yields an
`unsupported` verdict that names it, and the run never falls back. The JSON
report adds `"engine": "core"` and a `core` object with strategy metrics.

`--core-sample N` (with `--engine core`) is a diagnostic for the core's
developers. After the search, it takes `N` of the expanded states, chosen
by the run's seed, and compares each one's successors with the reference
evaluator's. The result is in the JSON report as `core.sample`. It changes
no result and has no TLC equivalent; TLC-equivalent runs leave it off.

Sampling retains each expanded state's actual level for `TLCGet("level")`
in its next-state relation. Both regenerated evaluations use that source
level, starting at one for initial states, so level-dependent successors
can be compared without an unavailable or invented level.

`--engine auto` (or `DIVE_ENGINE=auto`) runs the core where it can. Where the
core refuses, auto runs the whole check again on the reference engine and
reports the reference result. The refusal can come before the search
(`static`) or during it (`dynamic`). The core's partial counts and output are
discarded, so results are never mixed. The report names the engine that ran:
- in JSON, `"engine": "core"` or `"reference"`, `"engine_requested":
  "auto"`, and `"engine_fallback"` with the `stage` and `reason` (or `null`);
- in text, a line `engine auto: ...`.

A check that ends before any search, for example at a false `ASSUME`, names
no engine. After a dynamic refusal, the reference run gets what is left of
`--time-limit`. Auto works the same way under `--compat=tlc`: the core runs
TLC's profile, and the reference engine runs whatever the core refuses.

Core constructs finite literals and mapped sets canonically by default.
Mixed structured sets, such as a set containing both a tuple and a record,
have order-independent identity and can appear in independently validated
counterexamples. Direct undefined scalar comparisons still report errors.
Large symbolic powersets, function sets and sequence domains stay lazy.
When an equality, set element, function key or stored state needs an unproved
symbolic identity, checking returns `unsupported`. It does not infer inequality
from the symbolic expression or report an implementation limit as a specification
error; independent counterexample replay follows the same rule.

`--engine core --compat=tlc` selects the existing TLC integer, name and
exploration switches. Compatibility is nominal: unsupported comparison shapes
are named refusals, and Core does not reproduce TLC's per-object construction
history. See [the compat gaps](../tlc-differences.md#compat-gaps).

The `DIVE_CORE_OPT` variable (for example `-fold,-delta-hash`) turns
individual strategies off (`-name`) or on (`+name`) for measurement. Every
switch must leave the results unchanged, including independently replayable
counterexamples when hash-consing is disabled. See `docs/developer/core/06-memoization.md` and
`docs/developer/core/m0-results.md`.

Default-profile repeated compound membership in an admitted normalized set
can use a bounded hash index. It starts on the second eligible lookup for a set of at
least 16 closed members, verifies candidate values structurally and retains
normal miss/error checks. Each worker reserves at most 4 MiB or one-sixteenth
of its existing memo allowance, whichever is smaller; this adds no separate
memory allowance. `DIVE_CORE_OPT=-member-index` disables the index for
measurement. TLC comparisons always retain their original checked search,
including when `+member-index` is requested. Scalar queries, finite-fuel
optional analysis, raw constructors, lazy queries and unproved values retain
their existing path. Optional analysis reserves no index storage.

In the TLC profile, `-memo` also disables reuse of checked set-membership
proofs. These proofs share the existing memo memory allowance and preserve
the same comparison refusals. Supported tuple comparisons reuse those proofs
without building temporary comparison maps. Model-value type checks reuse
each worker's immutable name copy in both profiles; TLC's typed model-value
rules and runtime-name refusals still apply.

## TLC checking entrypoints

The checking adapter accepts either `dive tlc [OPTIONS] SPEC[.tla]` or the
installed `tlc [OPTIONS] SPEC[.tla]`. An executable whose basename is `tlc` or
`tlc.exe`, including a renamed binary or symlink to `dive`, selects the same
adapter. `cargo run -p dive-cli` still selects `dive`; both binaries share one
command implementation.

```sh
dive tlc docs/user/examples/Counter -seed 7 -fp 0 --output json
tlc docs/user/examples/Counter.tla -seed 7 -fp 0 -workers 1
```

The adapter selects TLC evaluation and TLC verdict statuses without requiring
`--compat=tlc`, and requests one worker unless `-workers N|auto` changes it.
`DIVE_COMPAT` and the native hidden chaos environment option do not change these
defaults. It accepts `-modelcheck`, `-config FILE[.cfg]`, `-seed N`, `-fp N`,
`-workers N|auto`, `-deadlock`, and `-h`/`-help`. Double-dash checking aliases and
`--output human|json` also work, including inline double-dash values.

Missing `.tla` and `.cfg` suffixes are appended. With an absolute module path,
an explicit relative `-config` is resolved from that module's directory. With a
relative module path, an explicit relative `-config` is resolved from the current
directory. The default configuration is always the module's sibling `.cfg`.
Absolute configuration paths remain absolute. There is no existence-based
fallback between these directories. Native file APIs keep their explicit path
semantics. Configurations embedded in `.tla` files retain their named refusal.

`-lncheck final` directly selects the existing bounded final checker and its
deterministic short lasso. It does not require a second policy flag. Reports
retain `tlc+overrides`, actual bounds, startup and planning errors, incomplete
capture and independent periodic-word validation. This selection need not match
TLC's arbitrary lasso choice. `-lncheck default` retains ordinary checking.
With profile-default resident-memory settings and `-fp 0`, eligible pure temporal
obligations use the same bounded final analyzer, reported as profile `tlc` with
actual final/short metadata and independent word validation. Other TLC-profile
temporal combinations retain named refusals; see [ordinary TLC-profile temporal
checking](temporal.md#ordinary-checking-in-the-tlc-profile) for exact eligibility.
Other strategies refuse by name. Native
`dive check --lncheck final --lasso short` keeps its existing explicit contract.

Every counterexample emitted by this adapter is independently replayed from
fresh source/configuration inputs. JSON records `counterexample.validation` as
`valid` only after replay succeeds. Unsupported replay returns status 3 and
invalid replay returns 4; both suppress the word and its violation claim.
Startup output and observed statistics may remain in the JSON failure report,
and are not proof of a completed check. Existing validator limits remain active.

Human output and versioned JSON use the existing check report. `-tool` message
framing is not implemented here. Finite `-simulate num=N` and `-generate num=N`
select the corresponding native sampling algorithm, with actual TLC exits and
independent sample validation; see [simulation](simulation.md#constructive-successors).
Unbounded/file sampling, DFID, checkpoint, storage, debugger, coverage and dump
flags retain named refusals. Unknown or malformed arguments exit 1;
recognized unsupported options exit 3 before input access. Input I/O errors go
to stderr and use 150 for the module and 151 for the configuration. The adapter
reads and parses the configuration first, as TLC does: a missing or malformed
configuration exits 151 even when the module is also missing.
`dive_api::check_file_tlc_inputs` provides the same input order for Rust callers;
filenames must already include their extensions.
Native commands retain their existing statuses and input ordering. These
entrypoints implement checking and finite sampling subsets; full TLC toolchain
parity remains open.

## Time and memory budgets

Core JSON metrics include `core.metrics.store.memory`: known persistent-store
allocation components and capacities, including unused slab slots and old index
tables still held by readers. `known_retained_bytes_lower_bound` excludes opaque
allocator, Arc and string-index bucket overhead, external shared objects and
other engine regions. Rehash scratch fields describe allocation traffic and
the largest individual temporary, rather than retained bytes or a process peak.
These diagnostics do not change the limits or replace the whole-process budget.

Every `dive check` runs under two budgets (ADR 0107):

- `--time-limit DURATION|none` bounds wall-clock time for the whole check.
  Durations are seconds by default, or take a unit: `500ms`, `90s`, `10m`,
  `2h`. The default profile defaults to 30 minutes, a provisional value until
  release; `--compat=tlc` has no time limit, as TLC has none.
- `--max-memory BYTES|auto|none` guards sampled memory of the whole process. Sizes take `-Xmx` units: `512m`, `8g`. `auto`, the default in both
  profiles, is half of physical memory.

The clock starts after the specification and configuration are loaded.
dive polls the budget for each initial state, each expanded state, and during
graph capture and temporal analysis. After startup, nested quantifier, CHOOSE
and filter predicate loops also poll at amortized work checkpoints, including
when no complete successor is emitted (issue #374). Successor domain candidates
poll before binding, so a rejecting continuation cannot bypass control. Action
composition also polls while collecting those candidates and before each
intermediate right step in next-state generation, action-predicate evaluation
and `ENABLED` (issue #393). These checks retain the original mode-specific
enumeration and counts. An interrupted private intermediate collection does
not become reported generated states; progress describes the externally
published prefix.

For example, this rejects candidates without producing successors and stops
with exit 5 under the explicit time budget:

```sh
dive check docs/user/examples/RejectedCandidates.tla --seed 0 --fp 0 --workers 4 --time-limit 100ms --max-memory 1g --output json
```

Fresh model binding installs the same run control after structural configuration
validation and real evaluator construction. Existing evaluator loop checkpoints
then cover constant startup evaluation, evaluator-backed role binding and action
expansion. An interrupted startup definition is returned as typed control flow;
it is never swallowed as a TLC evaluation error or relabelled `unsupported`.
A binding stop reports `model-binding`, retains output already emitted, and
reports no search statistics or used workers. The additive Rust
`dive_model::bind_controlled`, `bind_with_names_controlled` and
`Recipe::bind_controlled` entry points expose this capability separately from
`BindOptions`; their uncontrolled counterparts retain their original behavior.

Optional worker-selection proof planning also preserves its original resource
or internal error through controlled binding, including preceding startup
output. The legacy boxed `bind` and `Recipe::bind` errors cannot represent that
exception channel and retain their fail-closed bug wrapper. Rust integrations
that need the original intrinsic cause should use the controlled entry points
or `Recipe::bind_fallible`.

Producer `ASSUME` evaluation in safety checking and graph capture uses the
armed startup control at the same amortized evaluator-loop checkpoints.
Interruption reports `stopped` in `check-startup`, retaining preceding output
without search statistics, a behavior graph or a model verdict. Genuine false
assumptions and evaluation errors retain their existing classifications.

Structural binding and evaluator construction,
post-binding symmetry preparation, value-only allocation/sorting/native
operations, normalization in existing quiet equality/ordering/hash units, and
diagnostic error/count and predecessor-path reconstruction after an already
deciding event remain outside this control (#349, #379). The diagnostic exclusion
affects only the reconstructing thread; other workers retain control. It does
not suspend ordinary checking.

Fresh independent replay of final and prefix native temporal candidates inherits
the same armed control, including evaluator-backed rebinding. Both supplied and
serialized native words must validate before a violation is certified.
Interruption there reports `stopped` at `trace-validation`, with real capture
evidence and optional validation progress, without an uncertified word. A stop
while rebinding has no passed validation checks or supplied state position.
Rebinding output is retained as validation evidence and is not printed again as
original execution output. General TLC/session interpretation and standalone
`validate-trace` budgets remain follow-ups. Independent validator `ASSUME`,
serialization, and supplied deferred providers retained by another evaluator
remain outside this control. Diagnostic suppression never pauses candidate
certification. No universal wall-clock deadline is promised.
Unavailable memory observations interrupt these loops with typed `DIVE-R0002`,
preserving that failure rather than inventing an exhausted resource.
Quiet value units defer calling the host control until they return: an
infallible value operation cannot substitute a transient interruption for truth
or cache it as a structural hash. Ordinary fallible enumeration and comparison
retain typed interruption, including function-domain lookup with 32 or more keys.
A genuine incomparable argument still receives its existing evaluation error or
named normalization refusal. These quiet value units remain part of #349.
Memory is the whole process's: resident
plus swapped pages on Linux, resident pages on macOS. Where it cannot be
measured, checking fails with diagnostic `DIVE-R0002`; it never substitutes
zero memory or silently disables the requested ceiling. An unavailable physical
RAM lookup also fails when resolving `auto`. `--max-memory none` explicitly
disables memory enforcement. `dive tlc` accepts the JVM spelling `-Xmx8g`, or
`--max-memory`.

A finite memory budget takes a native observation before model binding and a
terminal observation before publishing a holding result, plus periodic samples.
An initial known excess returns `stopped` at `check-startup`, with no search
statistics or capture claim. Source/configuration loading errors remain earlier;
future binding effects, assumptions and initial predicates are not evaluated
after that cut. A terminal resource excess reports phase `finalization`,
retaining completed capture and progress without publishing a holding result.
The owner checks its elapsed clock at that boundary even if the watchdog was
delayed. macOS uses a native RSS query instead of launching `ps`.

`resources.memory` reports the platform metric (`rss+swap` on Linux, `rss` on
macOS), successful sample count, observed maximum, initial/final sample flags,
and unavailable status. The observed maximum is **not** the true process peak.
Sampling is not an allocation-time hard cap: excursions between observations
remain possible. macOS RSS does not claim swapped/compressed-memory or physical
footprint accounting. Earlier genuine errors and independently validated
counterexamples retain precedence if a later terminal observation fails.
See [ADR 0109](../decisions/0109-native-memory-budget-sampling.md).

When either runs out, dive stops and reports `verdict: "stopped"` with a
`stopped` block (resource, limit, observed value, elapsed time, unit, phase and
a hint),
the statistics reached so far, and exit status 5. A stop proves nothing about
the model: it is never a success, a violation or `unsupported`. Rust callers
set `CheckRequest::time_limit` and `CheckRequest::max_memory`
(`dive_api::BudgetSetting`). `options.budget` reports the resolved limits and
whether each came from the default, an explicit setting, or none. See
[D-026](../tlc-differences.md#d-026-run-budgets-stop-a-run-with-a-structured-result).

```sh
dive check docs/user/examples/Counter.tla --time-limit 2m --max-memory 4g --output json
```

## State and depth budgets

`--max-states N|none` bounds discovered in-model state identities, using the
same VIEW, symmetry and fingerprint/exact identity as the reported distinct
count. `--max-depth N|none` bounds BFS discovery depth; initial states have
depth one. Both are unset by default, which is their TLC-equivalent setting.
Zero is a valid explicit budget. TLC's simulation `-depth` option does not set
an exhaustive BFS discovery bound.

```sh
dive check docs/user/examples/Counter.tla --max-states 3 --output json
dive tlc --max-depth 3 --output json docs/user/examples/Counter
```

Both examples stop with exit 5 before discovering the fourth state; neither
returns a counterexample. Raise the ceiling to five to finish this finite
counter normally.

The first new identity that would exceed either ceiling is left unadmitted.
Duplicate identities do not consume the state budget. These limits bound
admitted identity discovery. If constraints exclude every initial candidate,
the empty retained graph can complete even with both ceilings zero; its
reported depth still follows TLC's existing depth-one print convention.
States at the depth boundary can still be expanded to already discovered identities, so a finite
graph exactly at both limits may finish normally. A truncated search reports
`verdict: "stopped"`, exit 5, resource `states` or `depth`, the attempted count
or depth in `observed`, actual admitted progress and a continuation hint. It
has no counterexample and makes no successful or temporal-completion claim;
its statistics carry `exhaustive: false`. The resolved limits appear in
`options.budget.max_states` and `max_depth`. An explicit finite ceiling labels
the TLC profile `tlc+overrides`; `none` restores TLC's unbounded setting.

Parallel workers retain their requested pool. Their generated candidates are
published in reference order up to the admission boundary, retaining earlier
errors and violations. Speculative candidates do not increase the admitted
distinct count, trace log or retained graph past the limit. Memory and elapsed
time remain separately governed by their run budgets. Recovery first checks
that its whole saved graph fits the requested ceilings before admitting it.

A prior genuine error or violation remains the deciding result. Diagnostic
error replay also stops admitting identities at the ceiling, and does not
check the invariants of an unadmitted state. Single evaluations and domains
materialized before the first emitted state still need their time or memory
budgets; discovery limits do not interrupt those operations. Rust callers set
`CheckRequest::max_states` and `CheckRequest::max_depth` (`Option<u64>`).

## Process statuses

`dive check` uses TLC process statuses by default, with native semantic choices
still active unless `--compat=tlc` selects TLC evaluation. An invariant violation
exits 12 and a rejected specification 150. Configuration rejection exits 151;
initial/action properties and supported cyclic liveness use 13. Deciding error
classes determine other verdict statuses. Success is 0; usage 2, unsupported or
resource limit 3, internal error 4 and a stop by an explicit or default run budget 5 mean their existing distinct outcomes.
These statuses do not prove an unverified error class or validate a missing trace.

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers 1 --output json
dive check docs/user/examples/Counter.tla --seed 7 --workers 1 --exit-codes native --output json
```

`--exit-codes native` keeps the historical CLI contract: invariant/property 10,
rejected input 1, deadlock 11, assumption 12 and evaluation error 13. Both inline
`--exit-codes=native` and separate arguments are supported; the last explicit
exit selection wins independently of `--compat` or `DIVE_COMPAT`. Native exits
with TLC evaluation are labeled `tlc+overrides`.

JSON adds the actual selected `exit_code`, `options.exit_codes` and
`options.semantic_profile`; `options.profile` includes compatibility overrides.
Human output displays the selected convention and process status. The existing
`tlc` object describes the deciding error class, independently of the selected
exit convention; `verified: false` stays unverified.

Under TLC comparisons, a source-qualified tuple/record equality error in an
initial invariant can report `TLC_FINGERPRINT_EXCEPTION` (2147), exit75. The
reference replays actual receiver construction, including a receiver rebuilt by
the first history-free Init emission, without constraints, VIEW or symmetry;
cached constants, later initial rows and stored Next values gain no source by
reuse. This changes diagnostic classification only. The default keeps D-009, and unproved
core comparisons remain `unsupported`. This recorded initial state is diagnostic
context, not a formal counterexample: schema 3 exposes it separately through
`diagnostic_witness`. Strict consumption requires independent replay of both the
named cause and the diagnostic wrapper. For example:

```sh
dive check conformance/corpus/comparison-receiver-primary/specifications/InitialStateLeft/InitialStateLeft.tla --engine reference --compat=tlc --seed 19 --fp 0
```

For source-qualified errors during Init, `tlc.code` can remain GENERAL (1000)
while `tlc.exit_status` is 75: TLC reports the original exception, then its
diagnostic replay determines the process status. Native override provenance
and the actual Init site distinguish these cases; error text alone does not.
Other sites and unknown causes keep their existing classification.

Rust callers keep `CheckReport::exit_code()`, `to_json()` and `render_human()`
behavior. To select explicitly, use `ExitConvention::Tlc` or `Native` with
`exit_code_for`, `to_json_with_exit_convention` and
`render_human_with_exit_convention`. Changing presentation does not rerun the
checker or mutate its evaluator or counterexample.

This applies only to `check`; other commands keep their documented statuses.
The named finite temporal 2116/exit 12 runtime boundary remains unsupported;
selecting TLC statuses does not implement that missing stream shape.

The explicit BFS checker loads a configuration, binds constants and roles,
evaluates assumptions, then explores supported initial and successor states.
It checks invariants, supported action properties, constraints and deadlocks.
`-deadlock` disables deadlock checking, matching TLC's spelling and meaning.
Use the sibling `.cfg` or select one with `--config FILE.cfg`.

A pure `SYMMETRY` declaration is prepared before assumptions and initial states.
Division by zero in that declaration and incompatible values encountered while
normalizing its set report `model-error`, TLC `GENERAL` (1000), with no search
statistics or counterexample. Both semantic profiles exit 255 by default; `--exit-codes native`
uses the historical model-error exit 1. The default comparison policy may produce
a different explanatory message for a mixed set, while retaining this class.
For example, this fixture fails before exploring any state:

```sh
dive check crates/dive-model/tests/fixtures/symmetry-group/specs/SyDivZero/SyDivZero.tla --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

Other unimplemented startup exception shapes remain named refusals. Effectful,
state-dependent and parameterized declarations still refuse; these diagnostics
do not expand supported symmetry modes or resource bounds.

Fingerprint storage is the normal mode. `--store exact` keeps canonical concrete
keys for supported values; fingerprint collision qualifications do not become
an exact-state proof. Inspect `statistics`, `collision`, `counterexample`,
`warnings`, `notes` and resolved `options` in the JSON report.

Supported pure parallel searches commit results in deterministic reference
order. Read `options.workers.requested`, `used` and `reason`: checker-register
reads and some modes can require an inline run. A requested pool width is not
a claim that every step used that many active threads. `--sequential` explicitly
selects inline checking. Simulation and checkpoint widths have separate limits.

Under TLC value or exploration order, runtime string creation can also require
an inline run. Concatenation alone does not add this restriction when a complete
source inspection proves that every reachable call has a tuple operand: literal
tuples, unchanged native `Append`, and transparent calls and branches can establish
the successful result kind. Argument expressions retain their own restrictions.
A closed operator-argument source path may forward unchanged binary comparisons
(equality, membership, subset and integer ordering); unknown callbacks retain
the restriction. Already-inspected configured literal values are call-free
leaves at zero-arity definition uses, without inheriting the replaced body's
result kind. Unknown state values, configured native origins, operator overrides
and opaque captured providers keep the conservative route. This proof does not
assume a type invariant, remove other sequential causes, or change evaluation,
error behavior or resource limits. Replay and checkpoint admission retain their
conservative source inventory. Requested and used widths remain reported;
the source proof is not a worker-utilization or performance measurement.
See [ADR 0114](../decisions/0114-proven-runtime-name-free-concatenation.md) for the
proof and source-inventory boundary.

Pure safety symmetry retains concrete counterexamples while canonicalizing
visited keys. Unsupported representative/startup/value interactions refuse.
Ordinary `RandomElement` checking supports a closed pure VIEW and pure startup/Init.
Expansion draws use the VIEW value's TLC fingerprint, while counterexamples retain
concrete variable values. Direct calls and proved configured native replacements
use the same authority. The runnable paired example
shows the reported seed, polynomial, requested width and actual inline width.
Unproved VIEW/startup contexts still return `unsupported` before binding, output
or search. The conservative source walk includes unused definitions and native
aliases, so importing unused Randomization bodies can also refuse this slice.
Concrete values and the VIEW value must fit 16,384 stored-value visits and depth
64: atoms, literal tuples/records, general functions with homogeneous atomic
domains, and explicit homogeneous atomic sets. Compound/mixed sets, symbolic or
deferred sets and nonliteral tuple/record representations remain named refusals;
these extra shape limits do not widen the unresolved mixed-word certificate
boundary in #190. No provider callback or comparison is used to grant admission.
Pure VIEW and direct source-call random checking without VIEW remain available.
VIEW normalizes all concrete variables before computing a visited key, even
variables the projection ignores. If a value cannot be normalized on the
proved bounded path, the result is `unsupported`; an unprintable state never
becomes a validated counterexample. See the measured boundary.
Random checking reports its actual inline width separately from requested workers.
Ordinary safety checking admits the configured `RandomElement` route, for example
`CONSTANT C <- RandomElement`, using the actual bound native operator for startup
and predecessor-stream selection. Native definition redirects and admitted typed
context aliases follow the same authority. These runs execute inline and disclose
requested and used worker counts separately. Pure user replacements still use the
parallel engine where admitted.

The committed configured-operator example is runnable directly:

```sh
dive check docs/user/examples/Stream.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

It reports an invariant violation with generated/distinct/depth `4/4/4` and
concrete values `0,80,94,100`, matching clean pinned TLC. Although eight workers
were requested, the report discloses the actual inline width of one.

A random violation on the first successor of an initial state
currently returns `unsupported` with its observed stopping-prefix statistics.
TLC's initial-predecessor printing path can keep an already-consumed stream and
omit the final state; dive has not proved that replay contract. The report contains
no counterexample certificate. The conservative source-wide random detector can
also refuse this boundary when an unused definition contains a random call.
This boundary applies in both profiles;
a new full-witness override has not been adopted. Startup draws before random Init can likewise
prevent reconstruction; a seed alone never creates an independent certificate.
The standalone validator also refuses unproved draw positions for multiple draws
or conditionally discarded random branches. A matching concrete value does not
establish where the generator consumed its stream. A checker violation whose
witness is missing or independently `unsupported` never counts as full agreement.
Configured `Randomization` replacements, SYMMETRY with randomness, graph
capture, temporal random checking and seeded native recovery remain explicitly
refused. The callback-free configured-target scan remains conservative across
matching modules and aliases. For graph capture and the search's random stream,
a specification counts as random when a definition in a non-standard module,
used or not, reaches a random operator through the definitions it references.
Extending `Randomization` or `TLC` without calling a random operator is not
randomness: the standard modules' own helpers, such as
`TestRandomSetOfSubsets`, count only when something outside them calls them.
See the original wrong-answer repair
and bound-stream controls and remaining replay limits.
The corrected standalone campaign independently validates 144 of 180 original
witness controls, refuses 36 unproved draw positions, and rejects all 180
changed-Init controls. The remaining refusals cover two draws, a discarded random
branch and conditional random Init. These counts belong to the corrected
producer; the original failed campaign's 48 refusals remain historical evidence.

Eligible ordinary dive-profile requests check temporal properties at deterministic
completed-level boundaries; ordinary TLC-profile requests use the bounded pure
final analyzer. Unsupported combinations retain named refusals. See
[temporal checking](temporal.md) for exact eligibility, actual schemas and limits. The explicit final/short mode
remains available in either evaluator profile. Refused library
operators, unknown finite deadlock covers, resource stops and internal failures
must never be treated as `success`.

See [compatibility](compatibility.md), [storage](storage-recovery.md),
[reports](reports-and-errors.md) and the [complete coverage ledger](../scoreboard.md).

A lazy filter ignored by VIEW does not require evaluating its predicate. The
checker normalizes its stored bound set only. `x = {i \in Nat : Print(i,
TRUE)}`, `Next == UNCHANGED x`, and `VIEW 0` finish with no Print output.
Spilling or archiving that callback-bearing value still refuses explicitly;
this support is limited to memory checking. Other unproved lazy representations
remain unsupported.

The current D-009 comparison improvements do not make every mixed structure
normalizable. The core refuses compound sets whose nested comparison shapes do not
unify in both profiles, including `Cardinality({<<1>>, <<TRUE>>})`. This
conservative refusal also covers some sets TLC can sort by an earlier field or
size without inspecting the conflict. `--engine auto` retries the whole check
on the reference engine. A reversed heterogeneous set may refuse even in the
default profile. A forward heterogeneous set can produce a checker report whose
canonical structured trace then fails decoder shape admission in both profiles.
Treat that report as an unvalidated witness; checking counts or a displayed
trace do not confer independent validation. See the measured selection and
refusals and
[follow-up #190](https://github.com/Chelis-Lang/dive/issues/190).


For automation, an invariant violation uses TLC's physical status (normally 12)
in both semantic profiles. `--exit-codes native` selects the historical status 10.
`--compat=tlc` also selects TLC evaluation. Conformance
compares these numeric statuses exactly. Known default differences count as
ExpectedDivergence only for declared exact pairs and valid independent witnesses,
never as Agree. See [reports and errors](reports-and-errors.md).

The conformance harness requires whole counterexamples for safety, action-property,
liveness and deadlock verdicts at every width. Missing or metadata-only words
receive no agreement credit; present words must validate independently. This
also holds when a worker-specific comparison omits witness shape.

If the host cannot start a requested budget watchdog, checking does not begin.
The CLI prints `DIVE-R0001` and exits 4; this is a runtime failure, not a verdict,
a refusal or exhausted time/memory budget. It is an admission error on stderr,
like an input-loading failure, rather than a completed JSON check report. Free
host resources and retry, or explicitly choose `--time-limit none --max-memory none`.
Managed `dive_api::check_file` callers receive `ApiError::BudgetStartup`;
`dive_check::check` returns an internal-error result, and liveness analysis returns
`AnalysisError::BudgetStartup`. Lower-level `Budget::start` and `budget::arm`
return `Result` so this failure cannot look like an unlimited budget. Already
armed or unlimited budgets still need no new thread.

Low-level `capture::*` functions expect the caller to arm a budget and retain
its watchdog through capture and analysis. Use the returned armed options from
`budget::arm`, or clone `watchdog.budget()` into the options: `Budget::start`
does not mutate the original budget. An unarmed budget has no running clock and
its polls return no exhaustion. These low-level primitives do not resolve CLI
defaults or arm a watchdog themselves.

The primary `interval-streaming` corpus records TLC's ascending, incremental
`IntervalValue` enumeration: `x # x` fails on the first initial state even for
a billion-element domain. A constant `FALSE` invariant instead fails configuration
validation before enumeration. The corresponding initial and successor witnesses
and a complete five-state interval are recorded separately from the established
conformance populations; see [the corpus guide](../conformance-corpora.md#interval-enumeration-resource-controls).

Unassigned `x \in lo..hi` initial predicates and `x' \in lo..hi` successor
assignments stream ascending integers without first retaining the whole domain.
This lets existing checker guards run when a complete assignment reaches the
sink. `ENABLED` likewise streams unassigned interval-membership candidates,
stopping at its first witness. Its membership-candidate loop reaches the
existing amortized evaluator control even when every candidate is rejected.
Already assigned variables still use the ordinary membership check.
Bounded evaluator contexts also stream primitive interval columns for
quantifiers and their shared binder traversal. Every domain is still evaluated
before the first body, including domains after an empty interval; the first
variable varies fastest and tuple validation keeps its original priority.
Existing loop checkpoints can therefore run without first retaining the entire
interval. Existing cardinality admission errors still apply before the first
body in both profiles. APIs requesting all bindings still collect the full
result. Other constructors can still materialize a domain before candidate binding, and long
individual evaluations remain potentially uninterruptible (issue #349). No fixed
domain-size count is introduced. Parallel checking polls the armed run budget during
successor emission (issue #362). A stop retains its actual ordered prefix and
leaves the unfinished source queued; a later worker's violation cannot bypass an
earlier incomplete expansion. A predicate such as
`x' \in 0..50000000 /\ FALSE` emits no complete successors, so the emission
guard alone cannot interrupt it. The additional amortized candidate checkpoint
now observes the budget before binding while this rejecting search proceeds
(issue #393), including inside private action-composition collection. Quiet
normalization/equality/hash units, domain materialization before the candidate
loop, and single long evaluations retain their limitations. For a bounded
first-state violation with an independently
validated witness, run:

```sh
dive check conformance/corpus/interval-streaming/specifications/WideIntervalInit/WideIntervalInit.tla --compat=tlc --time-limit 1s --max-memory 1g --output json
```

The invariant fails on `x = 0` (TLC class `TLC_INVARIANT_VIOLATED_INITIAL`, exit
12); this is not a successful proof over the rest of the interval.

A private set filter over a concrete source can pull candidates in original
order, evaluating its original predicate only when the next candidate is needed.
An accepted empty set reaches the continuation before a later failing predicate.
Each required component is still sized before its first output. A powerset
whose BASE size is zero yields its single empty subset before converting or
normalizing BASE; a nonzero BASE keeps the required normalization and its errors.
A nested powerset avoids sizing the outer aggregate. Unknown providers,
foreign evaluator captures, cached deferred history and cross-order components
keep their existing routes. This changes storage and effect timing to the
ordered enumeration, not predicate admission: no candidate is pruned or assumed
Boolean/total, and a very large rejected domain can still exhaust the run budget.

For an independently checked, closed predicate of the general form
`{A \in SUBSET B : \A b \in A : P(b)}`, the cursor may retain only base
indices whose original singleton predicates succeeded. It first evaluates and
emits the original empty candidate; every singleton then runs the original
whole predicate in order, and every accepted candidate reaches the original
sink. Metadata inspection supplies neither Boolean results nor totality.
Only the completed original singleton evaluations establish those facts over
the retained finite base. Larger retained candidates still run the whole
predicate. Their order is the same size/lexicographic subsequence.

Skipping a position additionally requires the current operative native-hook
policy to omit permission callbacks in ordinary cache domain zero. A host
replacement, changed checked-depth context, unknown mutable capture, provider,
outer-set dependence or unproved scope keeps ordinary enumeration. The private
position commit changes integer cursor storage only; predicate evaluation,
control callbacks, value construction and sinks remain outside the host lock.
No cardinality limit, explicit `Cardinality` behavior or reference result changes.

Compound assignment domains with recursively concrete finite components
also yield candidates incrementally: powersets, Cartesian products, function
sets and record sets reach the initial/successor sink and `ENABLED` continuation
without retaining every output. Required nonempty component normalization still
precedes the first candidate. For example, `SUBSET ({1, TRUE} \X {})` yields the
empty set without comparing the dead mixed column; an initial invariant can
therefore fail on that actual state. BASE size errors and nonempty conversion
errors remain unchanged, including their existing named class refusals.
Candidate checkpoints also run when a continuation rejects
every value, allowing the configured run budget to stop that search.

This changes output storage, without imposing a new cardinality limit. Ordinary
normalized bulk operations, deferred component providers and values requiring a
different exploration order use their existing paths. A compound component may
still require materialization; this does not make every domain construction
interruptible. The four pinned first-state controls and their independent trace
validation are documented in the [corpus guide](../conformance-corpora.md#compound-domain-first-state-controls).

Temporal analysis borrows the captured graph's edge targets and roots while
constructing labels and products. It retains every captured state and original
concrete successor occurrence. This avoids a second behavior adjacency copy; it
does not change capture, graph-label, product or acceptance limits, and does not
provide disk-backed temporal capture.

The [source-bound qualification](../survey/data/borrowed-topology-qualification-2026-10-23.json)
preserves all 312 small-tier model runs and 458 SANY agreements. Its eight normal
instruction pairs stay within 1% overhead (maximum 0.0301%). Four ordinary-release
CoffeeCan observations are descriptive; they establish no quantitative memory
or throughput gain. All measurements run dive, including its TLC compatibility
profile; this adds no capacity or parity claim.

The `liveness-atom-error-timing` validation microcorpus records four pinned
primary observations. The delayed-source error reports four distinct/generated
states, two queued states and depth three. The combined-claim case still evaluates
the bad atom and reports two distinct/generated states, one queued state and depth
two. Both report division by zero (2179), exit 255. The residual-tableau Init case
reports GENERAL (1000), exit 255, with one distinct/generated/queued state and depth
one. The action/state input is rejected by SANY (3002), exit 150: its bare action
is not allowed after `<>`, so it establishes no error priority. Its original
input and generated reference are retained. A separate
`liveness-atom-action-error-order` corpus uses `<<ActionBad>>_x` and reports
the state Concat error (2182), exit 255, with two distinct/generated states, one
queued state and depth two. This formula establishes no global action priority.
The exact manifests retain unknown metadata; these validation populations do not
change established small/full selections. The historical primary measurements
alone do not validate a dive error prefix or qualify an ordinary error verdict.

See the [compact primary observations](../survey/data/liveness-atom-error-timing-primary-2026-10-24.json) for input and generated-reference identities, exact outcomes and limitations.

The `92a33e3b` ordinary comparison records two agreements and two refusals in
this four-model corpus, in both profiles. `DelayedSiblingStateError` now matches
2179/255 and its partial counts with independent finite endpoint validation;
the invalid bare-action model already agrees on its SANY rejection.
`CombinedUnusedAtom` and `TableauInitAtomError` remain unsupported. Together with
the separate Concat and bin-priority controls, the five new agreements are
outside the established small and full populations. The
[qualification](../survey/data/temporal-row-errors-qualification-2026-10-25.json)
retains the tested source and unchanged baseline results.

The Rust detailed temporal-failure APIs retain a state atom's original typed
cause separately from its bound predicate source. `state_evaluation()` identifies
whether that failure occurred in final graph labels, a completed-level periodic
check, or candidate attribution; a synthesized nonboolean result remains distinct
from a core evaluator error. This evidence does not certify TLC's capture-time
error demand or stopping prefix. APIs accepting a caller-owned `&Model`, and detailed APIs retaining finite
`GraphLabelPolicy::Cells` (including their defaults), keep this posthoc refusal
behavior. A `CaptureRow` site is accompanied by separate
opaque `capture_row()` evidence; the site or typed cause alone grants no verdict.

A separate opt-in Rust capture API, `capture::explore_checkpoints_with_evidence`,
exposes actually reached Init and completed-source-row checkpoints. Its W1-only
inline execution is reported explicitly; ordinary capture and the existing
whole-Init/completed-level observer callbacks keep their behavior. The API can
retain a genuine core `ErrorClass::Eval` cause with real stopping counters and
a checked finite diagnostic source path. Unsupported, resource and internal
causes keep their existing mappings without a typed decision. That path is not a validated temporal counterexample, and the
checkpoint does not certify that TLC demanded an atom or would report that
error. The certified consumer below uses this API only after completing its
separate pre-capture proof. Parallel requests to the opt-in checkpoint API still
refuse before search; existing ordinary parallel routes keep their old behavior.

### Certified current-state row errors

A single-worker source-bound temporal request can report an original 2179 or
2182 evaluation error at the real completed-source boundary. Admission requires
an exact ordered TLC bin program with no residual tableau, complete must-read
coverage of every backend and attribution predicate, and a separate proof of
stable producers, captures, intermediate values and caches. This route requires
the existing `NoCellLimit` graph-label policy used by ordinary file requests;
finite cell policies keep their old admission before any label evaluation. The proof inspects
representations without running a predicate or provider. Unknown deferred values, nonliteral function representations,
normalization-sensitive operations, host effects or incomplete coverage retain
the old route before any new atom call; no model names or invariants grant admission.

Successful source rows retain actual Boolean results for analysis after capture
completes, preserving original vertex and edge-occurrence identities. Every due
periodic prefix check keeps the original graph, including its frontier labels
and attribution; it does not consume the row cache. An unpromotable semantic
atom failure discards all optional cached truth and continues the same capture
with the original label schedule. Resource, interruption and internal failures
remain stops. Completed final attribution uses cached results only while that
cache remains active.
Ordinary error reporting additionally requires the capture producer's matching
core cause, original source prefix and statistics, followed by fresh independent
validation of the finite behavior and an independently reconstructed ordered
error endpoint on another fresh runtime. Unknown independent validation
capabilities remain named refusals; allocation and budget failures remain
resource exceptions/stops. The report uses
`temporal-state-row`; it does not claim a lasso or violated property. Scheduling,
existing owner limits and JSON schema 3 are unchanged. Budgets, nonboolean results,
foreign contexts and internal failures do not acquire this error authority.

Temporal preparation reports TLC's zero-solution startup error (2253/77)
when the complete admissible conversion of the actual bound temporal roles,
combined with fairness, becomes literal `FALSE` under TLC's negation,
simplification and DNF rules. This happens after binding and its output, before
`ASSUME`, Init or exploration. Counts, property and counterexample remain null;
no state row, holding result or later atom error is substituted. A logically
true formula alone is insufficient: `<>TRUE` and `<>[]TRUE` retain their ordinary
analysis route. Effectful or unknown conversion scopes can still be refused.
The original configured claims remain separate attribution inputs; nonzero
startup preparation retains their existing backend and row-proof requirements.
This adds startup-only preparation work, not work per state or a performance
claim. These capabilities do not by themselves establish a new small/full
baseline or agreement count.

The
[typed state-error qualification](../survey/data/temporal-state-error-evidence-qualification-2026-10-24.json)
records the full gate at `2373b961`: 2,374 tests passed with none skipped,
the cm1 and validation-smoke gates passed at 1, 4 and 8 workers, and both
one-worker 156-model small-tier baselines retained every observable. TLC-profile agreement remains 150/156,
with 458/458 SANY module comparisons; default-profile agreement remains 144/156
with seven expected divergences and five refusals. No full-tier or performance
improvement is inferred.


### Native host bindings for embedding

The existing evaluator `set_host` and `replace_host` methods always invoke the
host's native permission callbacks. An explicit `native_hook_scope` can select
`Unrestricted` only with a host in shared ordinary cache domain zero and an
available generation. Otherwise selection returns `None` and changes nothing.
There is no fresh unrestricted binding constructor. The policy omits only that callback, while
print, register, introspection, action-composition, I/O and cooperative-control
checks remain in their original paths. Choosing this policy does not certify
purity, ownership, or permission to skip an evaluation. A caller must own host
replacement while using a binding scope; it does not lock out evaluator clones.
Domain zero deliberately shares ordinary cached values across mode restoration;
Callback restoration is not revocation of that authority. Independent nonzero
guard/replay domains always retain callbacks, so an unrestricted ordinary warm
cannot satisfy their permission-bearing cache entries.

Fresh ordinary checking uses this policy only with its built-in host, whose
native callback already returns success. It preserves binding state and output.
Fresh owned temporal capture keeps its existing fresh-host reset before
assumptions and Init. Public borrowed capture and legacy host setters retain
callbacks; diagnostic replay temporarily restores them too. Complete scopes
restore the host owner, policy and installed expression-cache domain with a new
nonreused installation generation. Exhausted generations grant no optional
admission authority. There is no new CLI switch or predicate-result cache.

Native compiled safety debugging is available through `dive debug` and public
Rust sessions. See [the debugging guide](debugging.md) for actual source
breakpoints, stop-scoped inspection and supported execution boundaries.
