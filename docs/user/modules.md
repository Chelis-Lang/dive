# Modules and evaluated operators

```sh
dive analyze docs/user/examples/ModuleExample.tla --output json
dive check docs/user/examples/ModuleExample.tla --compat=tlc --seed 7 --fp 0 --workers 4 --output json
```

This model imports `Naturals`, `Sequences` and `FiniteSets`, then checks an
invariant using arithmetic, `Len` and `Cardinality`. Module resolution and
operator execution are separate contracts: a module can parse and resolve while
a particular operator, argument shape or checking mode remains unsupported.
Successful analysis alone does not establish an executable model.

The native evaluator implements supported arithmetic, sets, sequences, records,
functions and selected standard/CommunityModules operators. Standard-module
source and TLC's native overrides are distinct: model binding decides which
implementation applies. Integer limits, comparison behavior and enumeration
order follow the resolved [compatibility profile](compatibility.md). An operator
name in the registry is an inventory entry, not a promise of complete support
for every argument and mode.

A constant definition whose startup evaluation fails remains lazy at its actual
use. For example, `F == [i \in 0..2 |-> IF i=0 THEN 0 ELSE 1 \div 0]`
can be selected as `F[0]` without demanding entries 1 and 2, whether the function
is named or written inline. Selecting `F[1]` or materializing the whole function
still raises division by zero. Successful startup values remain cached; genuine
resource failures and interruptions propagate instead of retrying. This rule is
the same in both profiles. A pinned runnable regression is:

```sh
dive check conformance/corpus/constant-retry-lazy/specifications/Named/Named.tla --config conformance/corpus/constant-retry-lazy/specifications/Named/Named.cfg --engine reference --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

`Len` accepts a function with an empty domain as the empty sequence, including
`[k \in 1..0 |-> 0]`, in both profiles and on the core. A nonempty function
must have domain `1..n`; an empty set or scalar remains an error. This checked
example runs without core fallback:

```sh
dive check crates/dive-api/tests/fixtures/core-len-empty/EmptyInterval.tla --engine core --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

Repeat `--library-path DIR` to search before bundled standard modules or
`--module-path DIR` to search after them. These directories contain TLA+ module
sources; accepting a source module does not load arbitrary Java plugins. Read
the resolved files reported by [analysis](front-end.md) when a local module and
a standard module have the same name. Pinned module libraries in the conformance
harness support oracle studies; they do not turn every library override into
native functionality.

The Core engine supports the CommunityModules graph overrides `AreConnectedIn`
and `SimplePath` in `Graphs` and `UndirectedGraphs`, plus `Graphs!IsStronglyConnected`
and `UndirectedGraphs!ConnectedComponents`. They work on finite, eagerly
enumerable graph records in both profiles. Singleton undirected edges are
self-loops; malformed edges and edges outside the node set add no connection.
Traversal uses checked membership and value equality, including compound nodes.
Unproved lazy conversions and comparison histories remain named refusals.

Both engines validate the graph's node and edge fields before returning an early
result. Explicit set elements are normalized only where the override demands
them. A node is connected to itself without inspecting edges, and an empty
directed graph is strongly connected without inspecting edges. A non-node source
can return false before comparing the target. This matters even for unused
incomparable edge values. `SimplePath` and `ConnectedComponents` still demand
their edge sets on empty graphs; demanded comparisons and invalid field types
retain their errors or named refusals. Formatting an invalid graph argument can
itself demand normalization, so an intended argument error does not suppress a
comparison needed by that diagnostic. The minimal state-dependent example now
succeeds in either profile:

```sh
dive check conformance/corpus/graph-demand/specifications/SelfDemand/SelfDemand.tla --library-path crates/dive-eval/tests/fixtures/community-modules --engine core --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

Normalized Java set conversions, including `SequencesExt!SetToSeq` and
`SetToSeqs`, check concrete Product emptiness before normalizing axes. This
follows the override's conversion-then-normalization order: an empty converted
collection leaves unused axes and their aliases untouched. Both engines run this
state-dependent control in either profile with three distinct/generated states,
depth three and success exit 0:

```sh
dive check conformance/corpus/set-conversion-demand/specifications/NormalizedProjectionSkipRaw/NormalizedProjectionSkipRaw.tla --library-path crates/dive-eval/tests/fixtures/community-modules --engine reference --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

Nonempty conversions still demand axis normalization. Later sizing of an
incomparable raw axis still errors, Product membership retains its original
receiver checks, and axis operand expressions are evaluated before conversion.
Normalization-first consumers such as `Functions!AntiFunction` still normalize
the original Product's axes before rejecting a non-function argument. That
receiver demand is distinct from normalizing an already converted collection.
Unknown earlier operands and providers keep their errors or named refusals. The
receiver error's native TLC source classification remains unverified; it earns no
error-parity credit.

Graph conversion also recognizes an empty Cartesian product before enumerating
its axes. Both engines support concrete explicit, bounded-interval and nested
product axes in source order, including products embedded in tuple and record
components. An empty axis leaves incomparable values in other
axes unnormalized; a later use of the same axis still makes its own comparisons.
Product operand expressions are evaluated normally, so an unused axis expression
that divides by zero still fails. Unknown earlier axes keep the existing guarded
conversion, and demanded incomparable nonempty products retain their errors or
named refusals. This model runs on Core in both profiles with three distinct and
generated states, depth three and success exit 0:

```sh
dive check conformance/corpus/graph-empty-product/specifications/LazyEmptyProduct/LazyEmptyProduct.tla --library-path crates/dive-eval/tests/fixtures/community-modules --engine core --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

Empty enumeration does not turn the original Product into an ordinary empty-set
receiver. For example, TLC still rejects `0 \in ((1..0) \X (0..0))` as non-tuple
membership. Core preserves that receiver through records, tuples, function values,
EXCEPT, `Append` and `:>` values. Non-tuple membership retains the demanded
evaluation error, including after graph conversion. It agrees with Reference's
exit convention (75); its TLC source classification remains unverified and earns
no native error parity credit. Nested product comparisons, general set elements,
function keys, state handoff and other unproved operator consumers retain explicit
refusals. This checked receiver control errors on Core in both profiles:

```sh
dive check conformance/corpus/product-receivers/specifications/BaseValidIntervals/BaseValidIntervals.tla --library-path crates/dive-eval/tests/fixtures/community-modules --engine core --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

`Cardinality` and `IsFiniteSet` accept complete concrete finite Product factor
trees: owned explicit sets, bounded intervals, powersets, function sets, record
sets and nested products. `IsFiniteSet` also preserves proved false answers for
known infinite Nat/Int factors, in either axis order and nested factor trees. For
example, `IsFiniteSet(Nat \X {x})` is false, while `IsFiniteSet(Nat \X {})` is true.
It follows source finiteness and emptiness demand in order; a known infinite axis
does not hide a later invalid operand. A function-set factor sizes its range only
when the source finiteness answer requires it. Cardinality follows the source
empty-axis, empty-function-domain and singleton-range short circuits, and
normalizes only demanded explicit-set sizes. The original Product remains
available for later receiver checks. Unknown demanded factors and sizes beyond
the proved signed 32-bit range retain named refusals. For example:

```sh
dive check conformance/corpus/product-operations/specifications/OrdinaryProductCardinality/OrdinaryProductCardinality.tla --engine core --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

This state-dependent example exercises every override on Core:

```sh
dive check crates/dive-api/tests/fixtures/core-graphs/GraphsStep.tla --library-path crates/dive-eval/tests/fixtures/community-modules --engine core --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

The exact pinned Examples Hanoi `Bits.class` has a native override for
exhaustive checking under the native worker/reference contract. Binding
checks the exact SHA-256 recorded in [provenance](../provenance.md#verified-local-class-overrides)
before replacing `Bits!And(x,y,n,m)` with signed 32-bit AND of `x` and `y`.
All four arguments must be TLC 32-bit integers, in both profiles. The class must
sit beside the resolved `Bits.tla`; without it, the TLA+ body applies. A changed,
unreadable or unknown class produces a named `unsupported` result. Configuration
substitution and arity errors retain precedence. The refusal occurs before evaluating constant
definitions, assumptions or configured predicates, so an unlike TLA+ fallback
cannot produce a verdict or error for an unimplemented class. Supported-class
configuration substitutions retain precedence. Native checkpoint/recovery records the exact
class identity; removing or changing the class rejects recovery. See [storage and recovery](storage-recovery.md).
Each supported native worker width compares to the pinned one-worker reference,
with exact required counts/depth, property, class, numeric exit and independent
validation of its actual concrete word. No-class fallback and general parallel
restrictions keep their existing behavior. Simulation and DFID retain their own
contracts; the historical guard is recorded in
[D-024](../tlc-differences.md#d-024-pinned-local-bits-checking-requires-one-worker).
This feature does not execute Java plugins.
From the pinned Examples checkout, the affected model can be checked with:

```sh
dive check specifications/tower_of_hanoi/Hanoi.toolbox/Model_1/MC.tla --compat=tlc --seed 7 --fp 0 --workers 1 --output json
```

`RandomElement` and admitted `Randomization` operations use the run's recorded
seed and mode-specific replay contract. Checker-register operations such as
`TLCGet` and `TLCSet` need runtime context and can change parallel eligibility.
A pure safety run admitting these operators does not mean simulation, temporal
capture, checkpoint recovery or independent trace validation admits them too.
Read each mode's supported/refused combinations.

An operator argument with a numeral or string literal as its entire body follows
TLC's startup substitution rules. For example, `LAMBDA x : 0` is passed to a
native library override as the value `0`. An override that requires an operator
then reports TLC's evaluation error in both profiles: `BagOfAll` reports 2266,
`SelectSeq` and `SortSeq` report 2169, and CommunityModules folds report 2154.
Bodies such as `x + 1`, `1 + 1` and `TRUE` remain operator applications; a
TLA+ definition that applies its operator argument also retains its ordinary
behavior. An empty `SelectSeq` does not apply its callback.

The enclosing evaluation context determines the deciding error class and exit
status. This consumed fixture calls `BagOfAll` in an `ASSUME`, so it reports
`TLC_ASSUMPTION_EVALUATION_ERROR` (2105), with cause 2266 and CLI status 10.
Its CommunityModules fixture sources require an explicit library path.

```sh
dive check crates/dive-api/tests/fixtures/op-literal/specs/OlBagOfAll/OlBagOfAll.tla --library-path crates/dive-eval/tests/fixtures/community-modules --engine reference --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

## Sequence replacement on the core

`SequencesExt!ReplaceFirstSubSeq(r, s, t)` and `ReplaceAllSubSeqs(r, s, t)`
run with `--engine core` on strings and on pure, materialized sequences whose
combined elements are all integers, all booleans or all strings. Empty
sequences are allowed. This holds in both profiles and at any worker count.
For sequence arguments, ReplaceFirst finds the earliest occurrence, including
when occurrences overlap.
ReplaceAll replaces every occurrence when none overlap; overlapping nonempty
occurrences remain unsupported unless its early pattern=text or
replacement=pattern CASE arm applies. TLC's remaining partition choice does
not always select the earliest absolute positions. For an empty pattern,
ReplaceFirst prepends `r`;
ReplaceAll inserts `r` before each element, without a trailing insertion. When
both pattern and text are empty, ReplaceAll returns `r`.

Mixed or nested elements, lazy sequence functions and sequence arguments with
effects, compiled calls or free local slots remain named `unsupported` results
on the core. Calls and free slots are conservatively refused because their
repeated-demand effects are unproved. In particular, the
TLA+ fallback can evaluate a sequence argument more than once; replacing it
with a single evaluation would change `PrintT` or checker-register behavior.
Existing effectful string arguments retain their single evaluation. Inputs and
materialized outputs are bounded at 2^24 elements. `--engine auto` reruns refused
cases on the reference engine; the default remains `reference`.

Auto preserves the frozen reference engine's result and output. For effectful
sequence arguments, that engine has a known reevaluation-order difference from
TLC (issue #612); equal output counts do not establish argument-order parity.
The core continues to refuse those calls.

This checked fixture needs the CommunityModules source path and completes with
success, 1 distinct state, 2 generated states, depth 1 and exit 0:

```sh
dive check crates/dive-api/tests/fixtures/core-replace/SeqReplacePure.tla --library-path crates/dive-eval/tests/fixtures/community-modules --engine core --compat=tlc --seed 0 --fp 0 --workers 4 --output json
```

The Rust API uses the same `CheckRequest.library_paths` and
`check_file_engine(&request, Engine::Core)`; see the
[Rust API guide](rust-api.md). The pinned semantics and remaining boundaries are
recorded in the [value-model notes](../developer/core/03-value-model.md#sequence-replacement-fallback-semantics).

## Pure JSON conversion

Pure `Json!ToJson(value)` has a bounded native conversion path. It uses the
bundled `Json` source and needs no file-access grant.
It converts booleans, integers, strings, model values, tuples, finite enumerable
sets and supported functions to compact JSON strings. Strings follow Gson
escaping without HTML escaping. Numeric `1..n` functions become arrays;
other supported functions become objects. The default profile uses canonical
object and set order under D-006. JSON strings can affect a specification's
predicates, so this order difference is observable.

With `--compat=tlc` or `--choose-order=tlc`, objects with two or more keys refuse
because normalized functions do not retain TLC's construction/normalization
history. A compound key containing a multi-field record also refuses, even
inside a tuple, stored set or function. Primitive keys and bounded compound
keys without that lost history remain supported. Recursive domain and value
conversion obey the evaluator depth limit. Symbolic/deferred domain-set printing
and infinite JSON-array conversion refuse explicitly. `ToJsonArray`,
`ToJsonObject`, `JsonSerialize`, `JsonDeserialize`, `ndJsonSerialize` and
`ndJsonDeserialize` remain named refusals. This pure conversion does not grant
host access or complete Json support.

State-time `ToJson` creates string names. When value or exploration order follows
TLC (`--compat=tlc`), checking reports the existing sequential worker fallback
so those names retain encounter order. The default ordering can use the
requested worker pool. This does not change JSON output or relax independent
trace validation's refusal for unknown post-startup name history.

The consumed `crates/dive-eval/tests/fixtures/json/JsonSupported.tla` and `.cfg`
retain the historical pinned-oracle example (`ToJson(<<"x", TRUE, <<1,2>>>>)`
returns `"[\"x\",true,[1,2]]"`). Run the fixture with:

```sh
dive check crates/dive-eval/tests/fixtures/json/JsonSupported.tla --config crates/dive-eval/tests/fixtures/json/JsonSupported.cfg --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

Focused native and pinned-oracle controls cover the supported conversion and
named refusals, alongside independent replay and malformed-word controls.
Replay accepts JSON strings whose names already exist in the bound model.
If a deciding word needs newly created JSON string names in the TLC profile,
independent validation reports the existing runtime-name refusal (#124); that
word earns no agreement. Conversion support does not remove this limitation.

### Host inputs and explicit grants

`dive check --allow-io=env,files,exec` grants named categories. Both profiles
start with no grants, including `--compat=tlc` (D-017). Denial happens before
native host callbacks. A grant supplies permission;
unsupported operators still refuse.

Ordinary Reference safety checking supports source-binding `IOUtils!IOEnv` and
successful `IOUtils!Deserialize(path, [format |-> "TXT", charset |-> "UTF-8"])`.
Use a constant-level definition to capture the input before assumptions and
state generation. For example:

```tla
Environment == IOEnv
Payload == Deserialize("input.txt", [format |-> "TXT", charset |-> "UTF-8"])
```

The file result has `exitValue = 0`, the exact text in `stdout`, and empty
`stderr`. Relative paths use the process working directory. Captured values can
be used throughout Init, Next and properties. Reads reached after binding,
non-UTF-8 input, file-error records, other serializers, writes and process
execution remain named refusals. An empty environment also refuses until its
empty native record representation is implemented. `exec` is accepted as a
category spelling but does not enable an executable kernel.
Only one reached TXT Deserialize request per fresh binding is admitted; reuse
its captured result. TLC shares mutable result-field names between calls, so
normalizing the first result can change later fields. Subsequent requests
refuse before arguments and callbacks in both profiles until that history is
proved (#761).

Auto selects the whole Reference backend before these effects. Explicit Core,
temporal-input production and checkpoint/recovery are not implemented. Simulation
and DFID do not expose input grants in this slice. A random seed alone cannot
recover external inputs.

Run this Bash example from a dive checkout. It builds the CLI, uses the checked-in
CommunityModules sources, and exercises both profiles in temporary directories.
Each check intentionally exits 12 with an `Inv` violation after three generated
and distinct states, at depth three. Each replay reports `"result":"valid"`
after the file is deleted and the environment values are changed.

```sh
CARGO_TARGET_DIR=target CARGO_BUILD_JOBS=4 cargo build -p dive-cli --bin dive --locked
dive_example_bin="$PWD/target/debug/dive"
dive_example_modules="$PWD/crates/dive-eval/tests/fixtures/community-modules"
dive_example_dir="$(mktemp -d)"

for dive_example_profile in default tlc; do
  (
    mkdir "$dive_example_dir/$dive_example_profile"
    cd "$dive_example_dir/$dive_example_profile" || exit 1
    cat > Settings.tla <<'TLA'
---- MODULE Settings ----
EXTENDS Naturals, IOUtils
Environment == IOEnv
Payload == Deserialize("input.txt", [format |-> "TXT", charset |-> "UTF-8"])
VARIABLE x
Init == /\ x = atoi(Environment["DIVE_COUNT"])
        /\ Environment["DIVE_LABEL"] = "outside"
        /\ Payload.exitValue = 0
        /\ Payload.stdout = "hello\n"
        /\ Payload.stderr = ""
Next == x' = x + 1
Inv == x < 4
====
TLA
    cat > Settings.cfg <<'CFG'
INIT Init
NEXT Next
INVARIANT Inv
CFG
    printf 'hello\n' > input.txt
    set --
    if test "$dive_example_profile" = tlc; then
      set -- --compat=tlc
    fi
    dive_example_status=0
    env -i DIVE_COUNT=2 DIVE_LABEL=outside "$dive_example_bin" check Settings.tla \
      --config Settings.cfg --library-path "$dive_example_modules" \
      --engine reference --allow-io=env,files --workers 1 --seed 7 --fp 0 \
      --time-limit none --max-memory none --output json "$@" > run.json \
      || dive_example_status=$?
    test "$dive_example_status" -eq 12 || exit 1
    cat run.json

    rm input.txt
    env -i DIVE_COUNT=99 DIVE_LABEL=changed "$dive_example_bin" validate-trace Settings.tla \
      --config Settings.cfg --library-path "$dive_example_modules" \
      --inputs run.json --seed 7 --fp 0 --output json "$@"
  ) || exit 1
done
rm -r -- "$dive_example_dir"
```

Repeat the original compatibility, configuration and module-path options. With
`--inputs`, the validator reads the report's native counterexample by default;
`--trace TRACE.json` selects a separate word. Replay uses fresh source bindings
and exact operation/path/order checks. Missing, mismatched and extra records
refuse. It performs no specification I/O, even if the original file was removed
or the environment changed. Reports identify external values as explicit
assumptions; validation proves their source use and word validity, not past host
authenticity. Native trace versions and report schema 3 retain their meanings.

The optional `external_inputs` report field records grants and raw inputs.
An `env` grant records the entire UTF-8 environment, including any secret values
it contains. One context permits at most 256 source operations, 1 MiB of total
UTF-8 input/path/name bytes, and 4,096 environment entries. The CLI admits input
reports up to 8 MiB. Individual operating-system reads are not cooperatively
interrupted; these are input bounds, not a hard wall-clock or process-heap cap.

TLC's IOEnv override interns keys and values in Java map order. Compatibility
mode therefore refuses a snapshot introducing more than one previously unknown
string (keys and values both count). It also refuses the first IOEnv reached
after a TXT request, because TLC constructs its static environment before file
results. A first IOEnv also conservatively refuses if startup evaluation has
created new strings since source planning; an earlier native call can change
ENV's initialization order. Capture the environment before constructing new
startup strings or requesting files. The canonical
default retains its existing deterministic order. The minimal `EnvironmentOrder`
and `TextBeforeEnvOrder` controls demonstrate verdict changes from unproved
construction order; broader demand-scoped ordering support remains open in #758.

Rust callers set `CheckRequest::host_access` using
`HostAccess::parse("env,files")`. For offline replay, call
`host_inputs::inputs_from_report(&report.to_json())` and supply the result through
`trace::TraceOptions::inputs`. Each `Session` attempt creates its own replay
cursor and evaluator; producer values and warmed caches are never proof.

For coverage details, see the [scoreboard](../scoreboard.md) and the
reference inventory in `conformance/parity-surfaces.json`. The
[maintained TLC differences log](../tlc-differences.md) describes approved changes
and known limitations. These inventories and curated examples do not certify
all standard-module or CommunityModules behavior.

`SVG!PointOnLine(from, to, segment)` has a bounded native path for literal stored
records with `x` and `y` coordinates and a segment, all 32-bit integers. Both
profiles wrap coordinate subtraction as signed32, then divide and add in
binary64 before truncating the result toward zero. The final cast returns zero
for NaN and saturates infinities or out-of-range values to the signed32 endpoints.
With segment zero, an unchanged coordinate becomes zero and a positive/negative
difference becomes the corresponding endpoint. This follows the installed Java
override and can differ from the module's TLA definition using integer division.

Each record has at most 256 stored fields; extra fields are ignored. Computed or
converted functions, deferred inputs, missing or noninteger coordinates and
unimplemented JVM argument shapes remain named refusals. The field bound and
configured evaluation depth remain resource limits. Argument expressions retain
left-to-right evaluation before representation checks, and an explicit TLA
definition override retains priority.

The consumed fixture uses a small SVG binding module and reaches an invariant
violation after two states:

```sh
dive check crates/dive-api/tests/fixtures/svg-point/Point.tla --module-path crates/dive-eval/tests/fixtures/svg-point --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

Its expected physical CLI status is 12; `--exit-codes native` explicitly selects
10. Rust `CheckReport::exit_code()` retains its profile-specific API contract.
This helper does not implement SVG element serialization, graph/ring layout,
host process access or animation completion.
