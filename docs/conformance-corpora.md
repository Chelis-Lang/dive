# Conformance corpora

The `construction-set-history` microcorpus pins twelve TLC models and twelve SANY
inputs for #623's scalar/model-value/nested-tuple literal, MAP and covered union
path. Initial primary observations were captured before runtime changes;
subsequent official regeneration records the final corpus digest and reason.
The reversed typed-model prefix errors while the early hit succeeds. Excluded
successors retain raw demand before fingerprinting; nested EXCEPT and a violated
invariant exercise actual state handoff and fresh independent word validation.
Empty record/set queries and a seen successor followed by additional emissions
pin empty-search demand and bound-root rollback lifetime.
Raw logs stay external. These controls establish neither a new corpus percentage
nor whole-model Sailfish performance; E7 diagnostic provenance remains guarded.

The separate `literal-set-history-primary` corpus contains 18 small
primary-first local set-literal controls for #623. Its generated goldens use
`tlc-2026.10.06.014338` at one worker, seed/fingerprint zero, with a recorded
generation reason before native runtime changes. They cover raw early hits
and incompatible prefixes, actual normalization, tuple length/first-field
short circuits, insertion permutations, duplicates, aliases versus equal
independent constructions, a safety word, and uncovered named/record/union
producers. Four primary evaluation errors are oracle facts only: the local
native lane refuses them without E7 provenance. Default D-009 controls are
kept separate from TLC facts. These controls do not enlarge the historical
156-model baseline or establish a full comparison/performance milestone.

## Next errors before temporal row completion

`next-error-temporal` pins four GenError models (record, tuple, general function
and nested function state representations), plus one SANY input. Before native
proof changes, the harness ran `tlc-2026.10.06.014338` at one worker and generated
all five goldens with a recorded reason. Every model reports division by zero
(2179), exit 255, two distinct/generated states, depth two and a two-state word.
The configured `P == []<>(n = 0)` remains present: it is not removed to replay
the finite error. Fresh replay must reconstruct the sole initial state and sole
first successor, establish total recurrences true on the completed row, and
execute the primitive primary and diagnostic error before the second row can
complete. This establishes no liveness verdict. Negative API controls keep
multiple emissions, false/erroring atoms, ToolRuntime and forged endpoints
outside the proof. The corpus is separate from historical baseline populations.

## Constant predicate binding capacity

`constant-predicate-binders` contains two finite Boolean models with a pure
5,000-element set constructor inside an always property. The pinned TLC oracle
accepts the holding property (generated 3, distinct 2, depth 2); it normalizes
the false property to an initial invariant failure of `P` (code 2107, physical
exit 12, counts unreported). This distinguishes admission capacity from actual
predicate truth and preserves the oracle's safety classification.

Generate these model goldens with:

```sh
target/release/dive-conformance golden generate \
  --corpus constant-predicate-binders --tier all --no-sany \
  --workers 1 --jobs 1 --xmx 512m --timeout 30 \
  --reason 'Requalify pure constructor capacity against the pinned TLC oracle'
```

The API controls additionally compare admitted planner and independent lasso
truth in both profiles, check exact class/status and holding counts, and validate
native and oracle witnesses against a fresh model. Forged initial states fail
independent validation. Effectful, checker-reading and infinite constructor
domains retain separate refusal controls. These two models are an explicit
local corpus; they do not enlarge the existing small-tier examples denominator.

## Generated ValidationTestSuite coverage

ValidationTestSuite at `fe99cff1ca48535bdbfc99fd95bd2706f056c12f` is a Python
pairwise-test generator, not a directory of ready-to-run models. Its pinned
source remains registered as `validation-test-suite`; the generated bounded
subset is registered separately as `validation-smoke`.

The selection combines case features `Eq`, `Let`, `InstanceNamedWith`,
`Enabled` and `BoolTrue` with plugs `BoolTrue` and `BoolFalse`. Both deadlock
settings are generated. It disables symmetry and anomalous-environment cases,
and requests upstream's reduced worker variants (1 and 2).

Every selected testcase is recorded in
`conformance/corpus/validation-smoke/selection.json`:

| Selected cases | Treatment |
| --- | --- |
| 16 single-worker TLC sides | Imported, with unmodified generated TLA+ and `.cfg` files |
| 16 alternate-worker variants | Explicitly out of scope for this single-worker-side import |
| 4 upstream skips | Retained with the generator's reason |

The raw upstream `specification.json`, generator recipe and dependency versions
are preserved alongside the census. Reference TLC/Apalache comparisons are
**not executed** by the import. Neither upstream's reduction relationship nor
its reference implementation is assumed to supply an oracle result. Our pinned
TLC supplies every model golden; SANY supplies every module golden. Model
manifests record expected result and runtime as `unknown`, putting them in the
deep tier without inventing runtime measurements or expected truth.

The first oracle run records eight successes, six invariant violations and two
action-property violations. TLC reproduces all 16 model goldens and accepts
all 20 modules. `dive check --compat=tlc` fully agrees on all 16, including
independent counterexample validation; `dive analyze` agrees on all 20 modules.
The model gate passes at 1, 4 and 8 workers. These are safety/action-property
cases; they do not demonstrate general liveness conformance. The separate
final/short study refuses these cases because they have no temporal properties.

The required `Conformance gate` includes all imported models using
`conformance/gates/validation-smoke.txt`. Nightly module checks include the
imported modules, and the deep tier includes their models in both profiles.
The import controls require that the gate list equals the imported census,
reject unhandled options/types, reject path escapes and digest changes, and
reject empty selections. Changing a recipe, model or census changes the local
corpus tree hash and requires golden regeneration.

## Reproduce

Fetch the pinned source with the harness, then use Python 3.9 (this run used
3.9.6) and the generator-only dependencies:

```sh
target/release/dive-conformance corpus fetch --corpus validation-test-suite
python3.9 -m venv .validation-venv
.validation-venv/bin/pip install -r scripts/validation-requirements.txt
PYTHONHASHSEED=0 PYTHONDONTWRITEBYTECODE=1 .validation-venv/bin/python \
  scripts/import-validation-suite.py \
  --source "$DIVE_CACHE_DIR/git/validation-test-suite/fe99cff1ca48535bdbfc99fd95bd2706f056c12f" \
  --out /tmp/validation-smoke-new
diff -rq conformance/corpus/validation-smoke /tmp/validation-smoke-new
python3 scripts/test-validation-import.py
```

Set `DIVE_CACHE_DIR` to the verified cache before these commands (the harness's
default is `~/.cache/dive`). The script requires a clean checkout at the pin,
works in a disposable source copy and never modifies a cache checkout. It never
runs the bundled upstream jars or reference checkers. Existing destinations
are refused; review regenerated content before replacing the tracked corpus.

After a reviewed corpus change, generate goldens and run the check:

```sh
target/release/dive-conformance golden generate --corpus validation-smoke \
  --tier all --jobs 2 --timeout 30 --reason 'Describe the reviewed corpus change'
DIVE_GATE_CORPUS=validation-smoke scripts/conformance-gate.sh \
  conformance/gates/validation-smoke.txt --workers 1,4,8 --timeout 30
```

Generated files are derived from NVIDIA's Apache-2.0 source. The source licence
and copyright are preserved in the corpus's `LICENSE.md` and `NOTICE`; the
adapter is original repository code. Coverage beyond this bounded selection,
symmetry/anomalous cases and the JUnit-driven TLC model corpus remain open in
issue #2. This import does not close that issue.

## INSTANCE query namespace controls

`selectseq-captures` is a separate local corpus of 19 tiny models generated
against the pinned TLC before extending source admission. Eight holds/fails
pairs cover callback binders, constructor/filter/recursive/nested-selection
captures, and lexical forwarding. Three controls retain unused effects or a
configured primed source: TLC leaves them lazy, while dive conservatively
refuses their capture contexts. All references report two distinct states,
three generated states and depth two. Run the explicit population with
`dive-conformance run --impl dive --corpus selectseq-captures --tier all
--no-sany --no-summary --timeout 30`; it does not change the established
small-tier population. Native positive cases also run with cold/warm caches
and fresh independent candidate replay in the liveness integration tests.

`selectseq-concat-type-capability` adds seven separate primary controls for
issue #424 before changing callback admission. The exact source manifests retain
`runtime` and `result` as `unknown`: editing these measured inputs would change
the corpus digest. This validation microcorpus changes neither the established
156-model small tier nor full-tier population.

| Control | Pinned TLC result | Distinct / generated / depth |
| --- | --- | --- |
| `CapturedConcatHolds` | Success, exit 0 | 2 / 3 / 2 |
| `CapturedConcatFails` | Property `P` violated, exit 13 | 2 / 3 / 2 |
| `ConcatFirstTypeError` | `TLC_MODULE_EVALUATING` (2182), exit 255 | 2 / 2 / 2 |
| `ConcatSecondTypeError` | `TLC_MODULE_EVALUATING` (2182), exit 255 | 2 / 2 / 2 |
| `EmptySkipsCallbackError` | Success, exit 0 | 2 / 3 / 2 |
| `StringConcatNames` | Success, exit 0 | 2 / 3 / 2 |
| `RightChildErrorBeforeKernel` | `TLC_MODULE_DIVISION_BY_ZERO` (2179), exit 255 | 2 / 2 / 2 |

The captured hold/fail pair retains an unused INSTANCE recursive-function
Concat source. Empty selection never invokes its bad callback. In the separate
right-child control, division by zero is reported before Concat checks its
invalid left operand: both actuals are evaluated before entering the native
kernel. The three error controls leave one queued state; the other runs leave zero.

The pinned `94d0c507` oracle generated these seven goldens with their recorded
reason, one worker, seed/fingerprint zero, a 256 MB heap and an explicit
20-second model timeout. All 11 selected clean/trace processes and 63 separate
setup subprocesses completed normally. The harness built at `1d11a2f0` retained
original setup/model evidence; raw outputs remain external.

The historical native comparison at `92bc7589` agrees on the captured hold/fail
pair and empty selection in both profiles. The failing word passes fresh independent validation.
The three evaluation-error controls remained `DIVE-L0033` refusals (issue #451),
and string concatenation retained `DIVE-L0020`: that was three agreements and four
refusals in each profile. A separate TLC-profile run of EWD998ChanID still refuses
VIEW graph capture (issue #428). These results add no agreement to the established
small or full populations. See the [compact qualification](survey/data/selectseq-concat-qualification-2026-10-24.json).
The 2026-10-25 comparison at `92a33e3b` reports **six agreements and one
refusal in each profile**. The three finite evaluation-error controls now match
the pinned classes, physical status 255, counts and depth, with fresh independent
error-endpoint validation. `StringConcatNames` remains refused. These isolated
results add no small- or full-tier agreement; see the
[current qualification](survey/data/temporal-row-errors-qualification-2026-10-25.json).
Select this microcorpus explicitly with
`--corpus selectseq-concat-type-capability --tier all --no-sany`.

`temporal-demand-primary-probes` records two further pinned references for
issue #451 before implementing a temporal error-demand consumer.
`UnusedDemandTautology` does not evaluate its bad state atom: TLC rejects the
property before exploration with `TLC_LIVE_FORMULA_TAUTOLOGY` (2253), exit 77,
all count fields unreported and no trace. Zero compiled solutions do not mean
a successful model check. `CurrentStateBinPriority` chooses division error
2179, exit 255, before the source-first type error 2182: TLC's compiled action
bin precedes its state bin even though both predicates read only the current
state. It reports two distinct and two generated states, one queued state,
depth two and a finite one-state error prefix `x = 0`.

The normal harness generated both references with their recorded reasons and
unchanged provenance: pinned TLC `94d0c507`, one worker, seed/fingerprint zero,
512 MB heap and 60-second per-model timeout. The source manifests retain
`runtime` and `result` as `unknown`. Before the consumer, ordinary comparisons
in both profiles reported two refusals: `DIVE-L0033` wrapped division error 2179
for the tautology and type error 2182 for the priority case. Those historical
results earned no error-parity credit.

The `92a33e3b` comparison reports **one agreement and one refusal in each
profile**: `CurrentStateBinPriority` now matches error 2179 and independently
validates its finite endpoint. Startup preparation now recognizes
`UnusedDemandTautology` through the actual bound temporal roles' combined
normalization, reporting 2253/77 with null exploration counts before assumptions
and Init. This is a separate startup failure, never a holding result or a
certified state row. Effectful conversion, including the conversion-time Print
reference below, retains its existing refusal unless conversion authority is
proved; the presence of a primary golden alone does not grant that authority.
Select this
separate microcorpus explicitly with
`--corpus temporal-demand-primary-probes --tier all --no-sany`; it changes
neither the established 156-model small tier nor the 265-model full selection.


Eleven additional primary controls for issue #464 pin startup normalization
priority before implementation. Five report 2253/77 with unreported counts:
false or printing ASSUME, failing Init, a false initial invariant, and a
conversion-time Print. The last retains `BIND_SEEN` then `CONVERSION_SEEN`,
without `ASSUME_SEEN`. A missing invariant instead reports configuration error
2229/151; constant division during conversion reports GENERAL 1000/255 with
nested cause 2179. Both orders of a tautological and a nontrivial property with
fairness, and the separate `<>TRUE` and `<>[]TRUE` controls, complete normally
at two distinct / three generated states / depth two. Logical tautology alone
therefore does not establish TLC's startup zero-solution branch.

These new goldens were generated by the normal pinned harness with a recorded
#464 reason, one worker, seed/fingerprint zero, 512 MB heap and a 30-second
per-model limit. The initial generation left the original two references untouched. After the
corpus membership expanded, the normal pinned harness regenerated all sixteen
references with an explicit provenance-refresh reason. Every result payload,
including those original two, remained identical; only provenance was refreshed.
[The compact primary record](survey/data/startup-tautology-primary-controls-2026-10-25.json)
pins all eleven inputs, outputs, classes and provenance. This adds references
before runtime work; it does not itself change established populations or
establish dive's reported agreement.


Three further primary role-splitting controls distinguish actual bound temporal
obligations from original configured property roots. Both split-root tautology
cases report 2253/77, including before a false ASSUME. An initial-only predicate
that would fail at a later state instead permits TLC to report a temporal
violation, 2116/13 for `P`, with two distinct / three generated states / depth
two and the word `x = 0`, `x = 1`, then stuttering. Its initial predicate is not
rechecked as a later temporal atom. These references use the same pinned
oracle and 30-second model budget and are recorded in the primary data's
additional-controls section; no runtime agreement is inferred.


At source `5217163f`, ordinary checking of all sixteen primary models records
**11 agreements and five named refusals per profile**, with no disagreements,
timeouts, pending validation or oracle errors. Seven pure startup-error cases
match 2253/77, including both split-role controls. The earlier finite2179 error
word remains independently validated. Effectful conversion, conversion error
class, both mixed-claim cases and initial-only later-row labelling remain
refused. The
[startup qualification](survey/data/startup-zero-solution-qualification-2026-10-25.json)
records the complete selection, original reports, full gate and unchanged
established baselines; this isolated population is not a full-tier result.

`instance-temporal` is a separate local corpus of ten finite Boolean models and
23 modules. It preserves primary-first observations of lexical INSTANCE and
`ENABLED` successor namespaces before runtime changes. Every source input is
bound by SHA-256 in
[`selection.json`](../conformance/corpus/instance-temporal/selection.json),
along with pinned TLC/JRE identities, the original clean primary outcome and
stream hashes. Original raw observations remain external.

| Models | Clean primary outcome |
| --- | --- |
| `guard_decoy_exported`, `guard_decoy_nested` | Success; generated 3, distinct 2, depth 2 |
| `conflict_same_direct`, `conflict_same_outside`, `conflict_no_root_nested`, `rebind_no_root_fresh` | Success; generated 2, distinct 1, depth 1 |
| `conflict_same_exported`, `conflict_decoy_exported`, `conflict_no_root_exported`, `rebind_no_root_old` | Initial invariant `P` failure; code 2107, physical exit 12; counts and depth absent |

All selected modules were accepted by SANY. The exported conflict results are
surprising: substitutions captured before the query do not identify its two
successor names. By contrast, the nested case shares the hidden name `q` and
remains unsatisfiable. The guard cases retain the actual current-state value; a
same-spelled caller variable does not replace it. The old/fresh pair distinguishes
bindings established before and after the query branch. These distinctions
prevent a blanket flattening of substitutions into caller root-variable IDs.
The pinned source explanation is recorded in [provenance](provenance.md#instance-query-namespace-controls).

Manifest runtimes are the original measured clean TLC process times rounded up
to whole seconds: all ten are `00:00:01`. These models therefore enter the small
tier normally. They are nevertheless reported as an explicit separate corpus;
the established 156-ID selection (examples, probes, validation-smoke and the two
configured-temporal-operators models) remains unchanged and separately reported.
Adding these controls does not increase its denominator or demonstrate a gain.
Gate lists and the original corpora and goldens are unchanged.

Set `DIVE_RUN_OUTPUT` to an external output directory. Generate only this corpus through the ordinary pinned harness, then self-check
the generated goldens with TLC:

```sh
dive-conformance golden generate --corpus instance-temporal --tier all \
  --workers 1 --jobs 1 --xmx 512m --timeout 30 --timeout-cap 30 \
  --reason 'Record lexical INSTANCE ENABLED namespaces before runtime implementation'
dive-conformance run --impl tlc --corpus instance-temporal --tier all \
  --workers 1 --jobs 1 --xmx 512m --timeout 30 --timeout-cap 30 \
  --no-summary --report-dir "$DIVE_RUN_OUTPUT/instance-temporal-selfcheck"
```

The harness supplies pinned Java and libraries, seed 0, fingerprint 0 and final
liveness checking. Its SANY process uses the existing fixed 1 GB heap; the TLC
heap above is 512 MB. Goldens preserve the harness's separate clean-result and
trace-pass consistency provenance. Internal transient Java streams are not
exported by this workflow; preserve the harness streams and physical status,
generated provenance, and already captured primary input/stream bindings.
A TLC self-check establishes oracle reproducibility, not native conformance or
independent validation of a counterexample. General fairness and closing-edge
word controls, callable substitutions, and uncovered derived-query errors remain
required beyond this initial selection.


## Full-tier primary reference recovery

The full tier is `--tier all` over `examples` (233 models), `probes` (14),
`validation-smoke` (16) and `configured-temporal-operators` (2): 265 models,
of which 48 Examples models are out of scope (for example symbolic Apalache
models). The original 263-model inventory, taken before the last corpus was
added, had 171 in-scope model references and 44 in-scope models without one.

All references for the 44 models were generated by `golden generate` with the
pinned TLC, the pinned Examples source, the authored configurations and model
modes, and one worker. Each golden records its own `--reason`, timeout and
heap in its provenance; no file was edited after generation.

- **First pass** (3 GB heap, 600-second limit per job): 27 references, 25
  exhaustive and two 100-trace simulations. They are 24 successes, one
  simulated invariant violation (`specifications/ewd687a/EWD687a_anim.cfg`,
  `InterestingBehavior`, a 26-state trace) and two classified initial-state
  errors (TLC code 2102, exit 255). A simulation success does not prove
  exhaustive correctness.
- **Second pass** (8 GB heap, derived limits capped at four hours) and
  **third pass** (6 GB heap, eight-hour limit) over the models still without
  a reference: exhaustive successes only.

| Model | Pass | Distinct states | Depth | TLC time |
|-------|-----:|----------------:|------:|---------:|
| `specifications/KeyValueStore/MCKVSSafetyMedium.cfg` | 2 | 17,220,672 | 22 | 23 min |
| `specifications/MultiCarElevator/ElevatorLivenessLarge.cfg` | 2 | 50,653 | 39 | 38 min |
| `specifications/MultiCarElevator/ElevatorSafetyLarge.cfg` | 2 | 59,007,145 | 44 | 42 min |
| `specifications/MultiPaxos-SMR/MultiPaxos_MC.cfg` | 2 | 37,078,209 | 37 | 77 min |
| `specifications/NanoBlockchain/MCNanoLarge.cfg` | 2 | 120,130,843 | 15 | 86 min |
| `specifications/bcastFolklore/bcastFolklore.cfg` | 3 | 501,552 | 9 | 68 min |
| `specifications/byzpaxos/PConProof.cfg` | 3 | 1,009,133 | 28 | 43 min |
| `specifications/SlushProtocol/SlushLarge.cfg` | 3 | 244,335,240 | 59 | 265 min |

The remaining 9 models have no reference and stay in scope, without
agreement credit. A run reports each of them as `oracle-error` (no golden):

- `specifications/CarTalkPuzzle/CarTalkPuzzle.toolbox/Model_3/MC.cfg`
- `specifications/KeyValueStore/MCKVSSafetyLarge.cfg`
- `specifications/TLC/TestMCReachability.cfg`
- `specifications/byzpaxos/BPConProof.cfg`
- `specifications/c1cs/c1cs.cfg`
- `specifications/dag-consensus/TLCSailfish2.cfg`
- `specifications/detector_chan96/EnvironmentController.cfg`
- `specifications/diskpaxos/MC_HDiskSynod.cfg`
- `specifications/ewd998/EWD998.cfg`

The full tier now has 208 in-scope model references (including the two
`configured-temporal-operators` models), 9 missing references and the same
48 exclusions. No model was excluded, resized or given a different
mode. The small tier and its agreement counts do not change.

Independent review of the first pass checked the 27 facts and their inputs and
reran six of the models directly with the pinned Java, including both
classified errors, both simulation modes and the exact violation trace.
`crates/dive-config/tests/tlc_oracle.rs` checks every model golden's
configuration, including the new violation's property name.

Twenty-five directories of the pinned Examples checkout have no
`manifest.json`, and four (`BlockingQueue`, `CCF`, `azure-cosmos-tla`,
`microwave`) are unfetched submodules; neither contributes models.

## Full-tier implementation measurement

The latest 2026-10-26 measurement of source `672fc1b1` records **182/217
in-scope agreements**, one more than the 2026-10-24 result. The only new
agreement is `examples:specifications/SpanningTree/SpanTreeTest5Nodes.cfg`,
with no lost agreements. The exact 265 historical IDs, 48 exclusions, nine
missing references and four corpora remain selected. The separate startup
primary corpora are outside this population. Sixteen named refusals, ten
resource stops and nine missing references remain; there are zero disagreements.

This retains TLC profile, W1/jobs2, native 16 GiB, TLC-derived budgets capped
at 900 seconds and 30 seconds of cleanup grace. Eight stops were for time and
two for memory, all native exit 5 with no completion credit. The normal harness
exit is 1 and nine references remain missing; this is not a gate pass.

After the normal report and `report baseline` utility completed, the controller
exhausted its S3 preservation window. That failure remains intact. Following
whole-node quiescence, preservation-only recovery and a RAM audit verified
all 1,288 originals (3,009,809 bytes), without rerunning models, builds or the
utility. `conformance/baselines/full-tlc-w1.json` is copied byte-for-byte from
that normal utility output for `20261026T023947024-dive-all`. The
[compact record](survey/data/full-tier-measurement-2026-10-26.json) binds the
source, complete population, every remaining gap, resource findings, generated
digest and S3/cleanup receipts. Small-tier baselines are unchanged. No new SANY,
four-worker, full-gate, throughput or peak-memory result follows.

### Earlier 2026-10-24 implementation measurement

The earlier 2026-10-24 measurement of frozen source `68c09a1b` reports **181/217
in-scope agreements** in the TLC profile at one worker: ten more agreements
than the controlled 2026-10-21 baseline, with no lost agreements. The fixed
265-model population, 48 exclusions, nine missing references and pinned oracle
are unchanged. Seventeen named refusals, ten resource stops and the nine
missing references remain unresolved; there are zero disagreements.

It uses two concurrent models, a 16 GiB native process-memory ceiling per model
and TLC-derived time budgets capped at 900 seconds, with 30 seconds of harness
cleanup grace. All ten timeouts are structured native stops with exit 5, nine
for time and one for memory. The [compact record](survey/data/full-tier-measurement-2026-10-24.json)
retains every gap's named cause and the stops' actual resource, phase and prefix
statistics. Signals and partial streams are absent; no stopped run earns
agreement or completion credit.

The original harness completed normally with exit 1, then the controller failed
with a post-run packaging `TypeError` after computing the report and raw audits.
The original failure remains intact. Following whole-campaign quiescence,
recovery ran only `report baseline` and preserved originals in S3, with no model
or build rerun. The generated digest `conformance/baselines/full-tlc-w1.json`
is copied byte-for-byte from that utility's output for
`20261024T165628670-dive-all`; it is not hand edited and no gate consumes it.
This is a measurement, not a new full-gate pass or release acceptance. No
full-tier SANY, four-worker, throughput or peak-memory result follows. The
separate small-tier baselines remain unchanged.

### Historical 2026-10-21 implementation measurements

The earlier controlled 2026-10-21 campaign compares frozen native source `39f4e483`
against the references above through harness `0ee2a2dd`. It is not a measurement
of later main: 171/217 in-scope models agree in the TLC profile at one worker.
All 265 historical `(corpus, model)` IDs are identical to the recovered
`652a5334` campaign: Examples 233, validation-smoke 16, probes 14 and configured
operators 2. The 48 exclusions and nine missing references remain selected.

The policy is explicit: two concurrent models, native whole-process memory
ceiling 16 GiB each, and `max(60 s, 10 × golden TLC duration)` capped at 900 s,
with 30 s cleanup grace. On the shared 64 GiB host this produces 171 agreements,
27 named refusals, ten structured resource stops, nine missing references and
zero new disagreements. Stops normalize to harness `timeout`; they never earn
agreement or become unsupported. The harness does not retain native progress
fields for stops, so no completion counts are inferred.

The historical campaign recorded 174 agreements, 28 refusals, five timeouts,
nine missing references and one kernel-OOM disagreement. Its original report
and classification remain intact. The new stop for `MCNanoLarge` is a different
observation, with no agreement or bug-fix credit. The three prior agreements
that now exceed the time budget, and EWD998Chan's refusal-to-stop change, are
listed with their issues in [the scoreboard](scoreboard.md#full-tier-measurement).
No model was dropped to improve the denominator and no golden changed.

The previous generated digest came from `report baseline` on report
`20261021T141221299-dive-all`; it remains in Git history.
The [compact evidence](survey/data/full-tier-budgeted-2026-10-21.json) records
both source pins, binary and report hashes, the exact selection hash, all ten
resource findings and the budget policy. Existing accepted small-tier baselines
remain unchanged. This incomplete result is not release acceptance; resource
and performance findings remain open.

## Interval enumeration resource controls

`interval-streaming` is a separate, explicit local corpus for issue #349. Its
seven models have pinned TLC and SANY goldens generated by the harness with a
recorded reason; they do not change the established small/full populations.

TLC reaches the first initial-state invariant failure for both `0..1000000000`
and the full signed 32-bit interval, without materializing the domain. The
invariant is state-level `x # x`: a constant `FALSE` invariant is rejected as
`TLC_CONFIG_ID_HAS_VALUE` before initial-state generation, retained as a separate
negative control. A wide successor interval reaches its first violating successor
with two distinct/two generated states and depth two. Complete enumeration of
`-2..2` agrees on five distinct/thirty generated states and depth one. These are
bounded semantic controls, not throughput measurements or a proof over a billion
states. Initial-state errors deliberately have no reported TLC counts.

The `ENABLED` witness control checks `1..1000000000` and succeeds at the first
candidate before an error in a later continuation: one distinct/two generated
states and depth one. The first-candidate error control runs inside a successor
assignment and preserves `TLC_MODULE_DIVISION_BY_ZERO` (2179), exit 255, one
distinct/one generated state and depth one. These goldens record ordinary query
semantics; they do not certify time-limit interruption or wide-domain throughput.
The first-error model remains `agree-pending-trace-validation`: its reported
observables match, but the independent validator cannot re-raise an error while
expanding the last state. That named gap earns no agreement credit.
Independent terminal-error certification is tracked in
[#434](https://github.com/Chelis-Lang/dive/issues/434). Both profiles agree on
the new witness and retain the first-error case as pending; neither model changes
the established small-tier denominator. The
[qualification evidence](survey/data/enabled-interval-qualification-2026-10-23.json)
keeps these observations separate from the full gate and normal-input overhead.

## State-time Json runtime names

`json-runtime-names` is one explicit validation model for #407, separate from
all established parity populations. It stores `ToJson(<<n>>)` while a numeric
state advances and checks the matching conversion. The reviewed pinned primary
run completed with generated 3, distinct 3 and depth 3; pre-fix native runs
matched those observations but omitted the TLC-order worker fallback. No
observable result disagreement is claimed.

Generate its golden only through the pinned harness:

```sh
target/debug/dive-conformance golden generate \
  --corpus json-runtime-names --tier all --no-sany --workers 1 \
  --jobs 1 --xmx 1g --timeout 15 \
  --reason 'Issue #407: record state-time Json conversion before correcting the native name-order guard'
```

The raw primary observations stay external; compact source and producer hashes
are recorded in [the primary evidence](survey/data/json-runtime-names-2026-10-21.json).
The native Array/Object conversions remain named refusals.

## Liveness quotient primary controls

`liveness-quotient-primary` is four explicit validation models, separate from
established parity populations. The two symmetry-disabled controls copy
`P4SymLive` and `SymLiveSymProp` without changing their module bytes; their
configurations remove only `SYMMETRY Sym`. The original reduced probes remain
unchanged. These controls establish primary references for a future whole-run
unreduced policy, rather than endorsing quotient liveness checking.

`ViewTwinTargets` has two configurations differing only in `Reverse`. They
reverse the generation order of concrete successors with the same VIEW. From
the second class these successors target the already-expanded initial class,
with different truth values for the single strong-fairness action `A`.
`WF(Next)` excludes permanent stuttering; `SF(A)` tests recurring action truth
rather than imposing two potentially contradictory retained edge labels.
These are order-sensitive primary probes, not proposed admission rules.

The pinned primary generation completed with a recorded reason, one worker,
`-lncheck final`, seed/fingerprint zero, a 512 MB Java heap, `--timeout 10`, and
a separate 15-second owned process-group backstop per harness arm. All four
harness arms finished uncensored. The timeout is an external harness process
budget; TLC has no native wall-clock budget here. Manifest runtime/result
metadata remains `unknown`; generated goldens hold the observed facts.

| Control | Pinned TLC result | Distinct / generated / depth |
|---|---|---|
| P4 without SYMMETRY | success | 2 / 3 / 2 |
| symmetric token property without SYMMETRY | success | 2 / 3 / 2 |
| VIEW forward successor order | success | 2 / 5 / 2 |
| VIEW reverse successor order | liveness violation of `Bad` | 2 / 5 / 2 |

Reversing the two equivalent disjuncts changes TLC's reported result. The
reverse result's matching trace pass supplies the literal two-state loop
`(x,y)=(0,0),(1,1)` closing to `(0,0)`. Source inspection shows that literal
repetition never takes `A` while `A` is enabled at every `x=1` state, so it
fails `SF(A)`. This is a source argument, not an executed independent validator
result. A matching trace pass is not a fairness validation certificate.
VIEW quotient capture, edge-label retention and concrete witness reconstruction
therefore remain separate obligations; these probes authorize no approximation.

[Compact primary evidence](survey/data/liveness-quotient-primary-2026-10-22.json)
records source, producer, pins and original observation hashes. Raw outer harness
logs and generated golden bytes stay external. This frozen generator does not
preserve its internal raw Java stdout/stderr; that limitation is explicit.

## Compound-domain first-state controls

`compound-domain-streaming` contains four explicit deep-tier controls for issue
#349, separate from established small/full populations. Each has invariant
`Bad == x # x` and a large lazy initial domain. Pinned TLC reaches the first
initial violation before constructing the complete domain.

| Control | Initial domain | First counterexample value of `x` |
|---|---|---|
| WideSubsetInit | `SUBSET (1..31)` | `{}` |
| WideProductInit | `(1..50000) \X (1..50000)` | `<<1, 1>>` |
| WideFunctionSetInit | `[1..2 -> 1..50000]` | `<<1, 1>>` |
| WideRecordSetInit | `[a: 1..50000, b: 1..50000]` | `[a |-> 1, b |-> 1]` |

All four generated goldens record `TLC_INVARIANT_VIOLATED_INITIAL`, property
`Bad`, native exit 12 and one finite state. TLC emitted no state counts or
depth; these fields remain null. The manifests' runtime/result fields remain
`unknown` because they belong to the exact pinned corpus digest.

Generation used the pinned oracle, one worker, seed/fingerprint zero, a 256 MB
heap and a 20-second explicit per-job timeout. All eight clean/trace native
commands finished uncensored. The opt-in raw-invocation harness retained their
original stdout/stderr, commands and traces. Its executable was compiled at
`bfa74020`; its production code is identical to qualified source `7218edde`.
No manual golden editing or new selected-corpus SANY run was involved.

The existing independent trace validator accepted all four original words and
rejected all eight mutations, including lossless format controls. These facts
precede the compound-assignment implementation and do not establish agreement
for unmeasured models or deferred provider effects.
[Compact provenance](survey/data/compound-domain-oracle-2026-10-23.json) records
source, input, original producer archive and exact generated-golden hashes.

`compound-domain-emptiness` separately pins two constructor-order controls.
The initial predicate first tries `x \in ({1, TRUE} \X {})` or
`x \in [a: {1, TRUE}, b: {}]`, then the disjunct `x = 0`. Pinned TLC skips
normalizing the mixed scalar component because the compound domain is empty,
then reports the initial `Bad` violation at `x = 0`. All four clean/trace runs
exit 12 uncensored; no counts or depth are emitted. These exact generated
goldens precede the corrected stream constructor. Independent pointwise Init
replay rejects these two generated words as non-tuple/non-record membership
errors; [issue #443](https://github.com/Chelis-Lang/dive/issues/443) retains that
validation finding. They receive no trace-validation or agreement credit. The original four large
controls and their corpus digest remain unchanged.
[Compact constructor provenance](survey/data/compound-domain-emptiness-oracle-2026-10-23.json)
records the new input and golden hashes.

The delivery qualification ran a fresh full gate (2,329 tests, no skips), the
unchanged 156-model selections in both profiles and all 458 SANY comparisons.
All four wide checks matched their primary words and independently validated.
The eight normal-model instruction comparisons retained identical counts/depth;
the largest overhead was 0.043%, below the 1% per-pair ceiling. This is capacity
evidence, without a TLC throughput or full-tier agreement claim.
[Compact qualification](survey/data/compound-domain-streaming-qualification-2026-10-23.json)
records source/tool identities, all eight instruction pairs and original hashes.


`nested-powerset-primary` is a separate local corpus of eight primary-first Init
enumeration controls for issue #459. They cover the first empty nested subset,
required component sizing versus explicit outer cardinality, accepted subset
order, and failing, nonboolean, effectful, outer-dependent and state-dependent
filter predicates. Descriptor result/runtime metadata remains `unknown` to
preserve the pinned input digest; the normal generated goldens hold the facts.

All eight references now generate against pinned TLC. Two first-empty controls
report invariant error 2107/12 before later candidates. Explicit outer
Cardinality and a nonboolean predicate report GENERAL1000/75 through the owned
Init-replay path; the required inner-component overflow remains distinct 2147/75.
The accepted-order, effectful and dependent controls succeed with distinct /
generated counts of 8/16, 4/8 and 5/10 respectively, each at depth 1. The six earlier
reference result payloads are byte-identical after normal regeneration. See the
[compact primary record](survey/data/nested-powerset-primary-controls-2026-10-25.json)
for source, oracle, result and preserved original-stream identities.

Select it with `--corpus nested-powerset-primary --tier all --no-sany`. This is reference generation before enumeration changes: no native
checker comparison or streaming/pruning capability is established. The existing
small/full selections and baselines are unchanged.


`empty-subset-base-primary` contains four isolated primary inputs for issue #478.
Normal pinned-TLC generation of all four inputs reports initial invariant
`Inv` violations (2107, exit 12) for both the mixed-product `EmptyBase` and direct
`DirectEmptyBase`, with the finite initial state `x = {}`. TLC does not report
state, generated, queue or depth counts for those violations; the generated
fields remain null.

The historical name `BaseSizeBeforeConversion` describes its original purpose,
not its observed error priority. Its mixed component produces GENERAL1000
"Attempted to compare boolean TRUE with non-boolean: 1", followed by
FINGERPRINT2147, exit 75 and zero counts. It is not evidence that a product-size
error is delivered before normalization. TLC's product-size overflow diagnostic
renders the product, and rendering an unnormalized explicit set can itself
normalize it. The observed mixed-comparison result is retained unchanged.

`BaseSizeOverflow` uses a homogeneous `{1, 2}` component with the same nonempty
interval. Its actual primary reports "Overflow when computing the number of
elements", wrapped as GENERAL1000 then FINGERPRINT2147, exit 75 and zero counts.
It establishes the required BASE-size failure independently of the mixed-column
comparison. All four references were normally generated with a recorded reason
under the new corpus pin; the first three result payloads are unchanged. No
native runtime fix is included.

Descriptor runtime/result fields remain `unknown` to preserve each input pin.
Select only this population with
`--corpus empty-subset-base-primary --tier all --no-sany`. The original eight-case
`nested-powerset-primary` corpus, historical pins and small/full baselines are
unchanged. The runtime correction skips conversion only after an actual zero
BASE size; nonempty errors retain their existing named class refusals. Native
qualification and whole-CLI comparison remain separate from these references;
no new agreement is claimed here.

## Assumption report locations

`assumption-reporting` contains five primary controls for false anonymous and
named assumptions, earlier-assumption priority, an assumption evaluation error
and a successful model. The pinned oracle yields three code-2104 failures with
source-image property names, one code-2105 error with no property, and one
success. All five modules are accepted by SANY. Generate with `golden generate
--corpus assumption-reporting --tier all --workers 1 --reason TEXT`; run with
`run --corpus assumption-reporting --tier all --impl dive:core --workers 1,4`.
The dedicated corpus keeps established corpus identities and denominators intact.

## Stored startup reads and configured redirects

`startup-stored-read-primary` contains one module with two configurations. Its
zero-arity `Draw` definition prints `DRAW` and selects a random value. `Direct`
reads that definition directly; `Redirect` substitutes it for the declared
constant `C`. Both use the same two-state relation and fairness condition.

An initial pinned-TLC probe at one worker, seed 0 and fingerprint 0 observed
success (2193, exit 0) with one `DRAW` for `Direct`, and a temporal violation
(2116, exit 13) with nine `DRAW` outputs for `Redirect`. Both reported three
generated states, two distinct states, an empty queue and depth two. The
contrast preserves TLC's distinction between reusing a stored startup value
and evaluating a configured redirect's body again.

Normal harness generation with a recorded reason now pins both results against
the oracle. `Redirect` retains the actual null violated-property name and a
two-state lasso returning to state one; its separate JSON trace pass matched
the clean run. These generated references preserve the actual result and
provenance rather than predicting them from the initial probe's output count.
[Compact provenance](survey/data/startup-stored-read-primary-2026-10-26.json)
records the input, corpus, harness, golden and retained original identities.

Descriptor runtime/result fields remain `unknown`. Select only these inputs with
`--corpus startup-stored-read-primary --tier all --no-sany`. This corpus changes
no evaluator or liveness behavior and establishes no native agreement. Existing
small/full selections and baselines are unchanged.

## Core definition identity across instances

`core-instance-scopes` contains two minimal nested-instance controls generated
with an official oracle reason before the #634 compiler correction. Holding
reports TLC success / 2193 / exit 0, 3 distinct / 4 generated / depth 3.
Violation reports an initial `Inv` failure / 2107 / exit 12 and a one-row word;
TLC does not print counts/depth for that initial failure. Native initial counts
remain separate, with `reported_by_tlc=false`, under the existing classifier.
Select this population with `--corpus core-instance-scopes --tier all --no-sany`.

Nine additional consumed compact oracle controls in
`crates/dive-api/tests/fixtures/core-instance-scopes/` cover reordered and lexical
uses, named outer actuals, plain/primed step definitions, supported identity
queries and demanded errors. Their independently checked words do not require
the separate #515 repair. The original Naturals pair retains #515's reference
failure on base `3dc31c0b`; its core words are independently checked by frozen
reference candidate `82b03ca0`, with the separate executable identity retained
externally. No reference runtime change is composed into this compiler lane.
These selected controls do not promote historical census/baseline totals or
establish general INSTANCE completion.

## Reference INSTANCE and native arithmetic

`reference-instance-naturals` pins the exact #515 reproduction: `In6` declares
`v` and `Double == 2 * v`; the root substitutes `v <- x` and checks the result
against `2 * x`. Official generation with a recorded reason gives TLC success,
2193 / exit 0, 1 distinct / 2 generated / depth 1. Select it with
`--corpus reference-instance-naturals --tier all --no-sany`.

`instance_naturals.rs` consumes that golden and 18 additional compact official
oracle controls in `crates/dive-api/tests/fixtures/instance-naturals/`.
Both profiles and 1/4/8 workers exercise actual arithmetic dependencies,
configured targets, nested scopes, constants, stored startup output, unused
actuals and named invariant-error causes. The original remains strict on
reference/core/auto; the nested-scope control exposes separate core/auto
disagreement #634 and earns reference agreement only. The assumption control
checks error priority while #504 separately tracks name metadata. No historical
baseline, whole-corpus population or milestone total is increased by these
bounded controls.

## Word-relative liveness attribution

`word-relative-liveness` contains three small exhaustive configurations and three
modules: the original multi-property `Liveness` reproduction, the minimal
`Mutual` distinct-lasso property sets, and `Identical` two-name formatting.
The six TLC/SANY references are generated from the pinned oracle with a recorded
reason. These controls exercise ADR 0126's fresh independent whole-word and
complete false-property attribution requirement; they do not refresh historical
population totals or relax any other conformance field.

## Failed constant pre-evaluation and lazy function selection

`constant-retry-lazy` retains four minimal named/inline, whole-function-error and fairness-action
controls. Pinned TLC may fail to pre-evaluate a constant function at startup,
then successfully select one of its defined entries later: it swallows the
startup failure and evaluates the original definition in the actual call-site
context (`SpecProcessor.processConstantDefns`, `Tool.evalApplImpl`, source
`94d0c50`). A named function does not acquire a new eager demand merely because
its startup evaluation failed. For example, selecting entry 0 from
`[i \in 0..2 |-> IF i=0 THEN 0 ELSE 1 \div 0]` succeeds; selecting a failing
entry or materializing the whole function still raises the demanded error.

The named and inline invariant controls both succeed with 3 generated states,
2 distinct states and depth 2. The named fairness-action control has a reachable
`x'=1-x` graph and violates `P` (physical exit 13). These observations are
separate from broader function-constructor or EWD capacity qualification. Model
goldens are generated through the official harness with a recorded reason;
raw clean/trace invocations remain outside the repository.

## Bounded temporal source-row errors

`fairness-primary` retains five classified one-worker GENERAL1000/255 controls:
the original Next-failure and safe-Next specimens, action-before-state priority,
a quiet action before a state error, and a complete self emission before a Next
failure. All require exact generated/distinct/depth facts, no named property and
an independently validated finite source word. The API fixtures and corpus
inputs are byte-bound in the reader integration test.

Four additional modules are SANY controls only. The Plain/OtherFair oracle
streams remain unclassified; DifferentPrimary and CandidateBeforeTemporal have
different retained-primary or terminal-word contracts outside this capability.
They must not acquire the new GENERAL source ending. Their existing configs are
kept as negative tests, with no invented model golden or agreement credit.
All five model and nine SANY goldens come from the official harness with a
recorded reason. Raw runs remain external. This adds no parity population or
performance acceptance claim beyond these selected controls.

The whole-function-error control forces `F=F` for that partial function. Its
fresh diagnostic receiver catches the underlying division2179 and decides
TLC_FINGERPRINT_EXCEPTION2147 with exit75, 1 generated and distinct state,
depth1 and one queued state. This differs from selecting the defined entry;
the source-bearing receiver, rather than the division's numeric code, determines
the wrapper. See [the source mechanism](survey/value-method-error-provenance.md).

The `comparison-receiver-primary` corpus records twelve TLC deciding models and
twelve accepted SANY modules for #620. Literal tuples/records, scoped LET/INSTANCE
construction and an initial-state receiver rebuilt during diagnostic Init replay
retain2147/75 under TLC comparisons. Direct integer and subsequent-invariant
errors are precedence controls; dynamic Next comparison remains unverified.
Seven additional context-only streams were deliberately not written as goldens
by the official generator and live as negative API fixtures. All primary results
were generated with recorded reasons; no context message was promoted into an
oracle fact. These selected controls do not refresh historical totals.

## Graph override demand

The separate `graph-demand` corpus records seven classified models and twelve
accepted SANY modules for #756. Pinned `AreConnectedIn` validates graph fields
and normalizes nodes before its membership/self short circuits, but does not
normalize unused edges. `IsStronglyConnected` similarly skips edges for an empty
node set. These early results can succeed with incomparable edge elements;
`SimplePath` still demands adjacency on an empty node set. Invalid-argument
formatting can itself demand a graph's raw values and fail before its intended
argument diagnostic.

The five successes and two independently replayable invariant violations have
generated/distinct/depth 3/3/3, with exits 0 and 12. Five context-only initial-error
streams were refused by the official golden generator and remain negative
configurations with no model golden or agreement credit. `DemandedEdges`,
`DemandedEndpoint`, `FieldPriority` and `SimpleEmpty` retain outer 2102/2103;
`SelfEdgeType` retains 1000/2103. All five physically exit 75. The complete
attempted selection is retained externally. The source/configuration controls
live in `conformance/corpus/graph-demand/specifications`; negative configurations
have `models: []` and are consumed directly by API tests. No historical census
or established corpus identity is refreshed by these cases.

The [compact correctness record](../conformance/reports/2026-11-03-graph-demand.json)
binds the frozen source/artifacts, both-engine/profile harness panels and the
verified external raw archive. It retains the earlier corpus-identity, stale
validator and converted-node receiver failures separately from passing results.

Regenerate the classified controls with the pinned harness, retaining raw runs
outside the repository when requested:

```sh
target/debug/dive-conformance golden generate --corpus graph-demand --tier all \
  --workers 1 --jobs 2 --timeout 30 --xmx 512m \
  --reason 'Refresh graph demand controls against the pinned oracle'
```

## Java set conversion and original-argument diagnostics

The separate `set-conversion-demand` corpus records concrete Product conversion,
raw-axis aliases, receiver demand and ordinary nonempty consumers for #770.
Conversion to a fresh empty collection does not normalize the original Product.
`AntiFunction` instead normalizes its original receiver before type rejection;
its comparison demand remains separate from conversion-first consumers.

`LongestCommonPrefix` converts and normalizes first, then prints the original
argument before constructing its empty-set error. Pinned `Value.toString` uses
selective swallowing: Product expansion selection catches sizing and conversion
failures, but rendering its fallback axes can still fail. For
`{x, TRUE} \X {}`, sizing fails, then fallback rendering demands the boolean
comparison. For `{} \X {x, TRUE}`, the size loop stops at zero and the converted
empty collection prints `{}`. Nesting the reversed Product also prints `{}`:
the nested Product's `size` checks emptiness before sizing its own axes.
Empty-first scalar and `Nat` tails are skipped. `Nat \X {}` uses the symbolic
fallback `(Nat \X {})` after its size failure is swallowed; Reference at the
pre-correction source retains its named conversion refusal in both profiles.

The separate `set-diagnostic-demand` corpus adds reverse, nested, unknown,
infinite and ordinary controls pinned before the diagnostic correction. Its
seven initial-error cases and the original `PrefixRawEmpty` forward control
remain context-only with `models: []`; no nested code or message text supplies a
deciding model class. The separate identity preserves all existing
`set-conversion-demand` golden bytes and provenance checks. `PrefixOrdinary` is a
generated primary success with three generated states, three distinct states and
depth three. See the
[compact oracle record](../conformance/reports/core/2026-11-04-set-diagnostic-oracle.json).
The [frozen correction record](../conformance/reports/core/2026-11-04-set-diagnostic-demand.json)
binds the runtime source, artifacts, focused checks and both-engine/profile panels.
This bounded addition changes no historical qualification totals or earlier
rejection records.
