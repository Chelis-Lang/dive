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

TLAPS and the CommunityModules TLA+ sources are bundled with dive. Resolution
searches the specification directory, explicit `--library-path` directories,
standard modules, explicit `--module-path` directories, modules inside the root
file, then the bundled libraries. Existing user sources therefore take
precedence. Bundled sources appear as `"origin": "bundled"` in analysis JSON;
standard modules retain their existing origin and priority. The source pins
and licenses are in [the bundled-library README](../../crates/dive-sema/bundled/README.md).

The bundled-library smoke checks ten original Examples without library flags.
Nine must check successfully on Core, the default engine, and every report must
name `engine: core`. `EWD998ChanID` resolves its imports but may report an
explicit unsupported result: Core refuses its VIEW/liveness graph capture by
name. This is a
checking limitation, not a missing-library error; it does not establish a
verdict.

The native evaluator implements supported arithmetic, sets, sequences, records,
functions and selected standard/CommunityModules operators. Standard-module
source and TLC's native overrides are distinct: model binding decides which
implementation applies. Integer limits, comparison behavior and enumeration
order follow the resolved [compatibility profile](compatibility.md). An operator
name in the registry is an inventory entry, not a promise of complete support
for every argument and mode.

Core resolves a configured constant such as `C <- Target` through imported
declarations inside `INSTANCE`, including nested instances. Explicit `WITH`
bindings still take precedence. The target keeps its own module and lexical
scope, while state substitutions retain their current or primed meaning.
The configured target's body runs when demanded; ordinary helpers retain their
successful startup values, including Print and RNG results. Startup
pre-evaluation has no checker-register writer: integer register writes fail
softly and remain lazy, while named writes publish no register value. ASSUMEs
and checking use their ordinary register access. Lazy function field selection
keeps the same startup availability rule. An explicit configured value
override still replaces the target. Lazy configured functions selected through
an instance can remain unsupported when Core cannot
preserve undemanded entries. Configured targets with unresolved recursive or
`ENABLED` calls also refuse. The refusal gives no checking verdict.

A constant definition whose startup evaluation fails remains lazy at its actual
use. For example, `F == [i \in 0..2 |-> IF i=0 THEN 0 ELSE 1 \div 0]`
can be selected as `F[0]` without demanding entries 1 and 2, whether the function
is named or written inline. Selecting `F[1]` or materializing the whole function
still raises division by zero. Successful startup values remain cached; genuine
resource failures and interruptions propagate instead of retrying. This rule is
the same in both profiles. A pinned runnable regression, which succeeds with 2
distinct states, is:

```sh
dive check conformance/corpus/constant-retry-lazy/specifications/Named/Named.tla --config conformance/corpus/constant-retry-lazy/specifications/Named/Named.cfg --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

`Len` accepts a function with an empty domain as the empty sequence, including
`[k \in 1..0 |-> 0]`, in both profiles and on the core. A nonempty function
must have domain `1..n`; an empty set or scalar remains an error. This checked
example runs without core fallback:

```sh
dive check crates/dive-api/tests/fixtures/core-len-empty/EmptyInterval.tla --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

`Seq(S)` keeps an original Cartesian-product range on Core. An empty sequence
does not inspect unused range factors; a nonempty sequence checks each actual
element through the original Product's membership method. This preserves
non-tuple errors, including when that Product is empty. Both profiles run this
state-dependent example without fallback:

```sh
dive check crates/dive-api/tests/fixtures/core-receiver-consumers/SeqProduct.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

`IsFiniteSet(Seq(P))` checks the original Product's finiteness, then its
emptiness only if finite. An empty Product range makes the sequence set finite:
it contains just the empty sequence. Unused axes remain untouched. The same
source method demand is supported through unary `Seq` and `SUBSET` wrappers;
other unproved compound finiteness demands and malformed factors refuse. This
checked example succeeds with one distinct state, two generated and depth one:

```sh
dive check crates/dive-api/tests/fixtures/core-receiver-consumers/SeqProductFiniteEmpty.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

Nested `SUBSET` membership can enumerate an owned raw literal query, preserving
its checked normalization before the first element test. An empty query leaves
the base and independent aliases untouched. Unproved raw receivers and demanded
mixed-kind comparisons remain explicit refusals. This also holds with the
internal symbolic-membership optimization disabled. The checked example is:

```sh
dive check crates/dive-api/tests/fixtures/core-receiver-consumers/MappedSubsets.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

Under `--compat=tlc`, an outer mapped collection containing multiple raw sets
still refuses when its constructor and comparison history are unproved.
`MappedMultipleSubsets.tla` beside these examples records that boundary. In the
default profile sets are canonical, and the original Examples
`LoopInvariance/MCQuicksort` model agrees with TLC on Core.

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
dive check conformance/corpus/graph-demand/specifications/SelfDemand/SelfDemand.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

Normalized Java set conversions, including `SequencesExt!SetToSeq` and
`SetToSeqs`, check concrete Product emptiness before normalizing axes. This
follows the override's conversion-then-normalization order: an empty converted
collection leaves unused axes and their aliases untouched. Under `--compat=tlc`,
Core refuses this mixed-shape control by name (TLC's checked comparison of a
set of values of different shapes is not ported):

```sh
dive check conformance/corpus/set-conversion-demand/specifications/NormalizedProjectionSkipRaw/NormalizedProjectionSkipRaw.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --workers 1 --seed 0 --fp 0 --output json
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

`LongestCommonPrefix` rejects an empty converted set after rendering its original
argument. For `{x, TRUE} \X {}`, that rendering still demands the mixed axis's
comparison and reports its error. An empty first axis, or a nested Product that
is already empty, prints `{}` without demanding unused axes. TLC exits 75 on this
example with the demanded boolean comparison; Core refuses the mixed-shape set
under `--compat=tlc` by name:

```sh
dive check conformance/corpus/set-diagnostic-demand/specifications/PrefixReverse/PrefixReverse.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

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
dive check conformance/corpus/graph-empty-product/specifications/LazyEmptyProduct/LazyEmptyProduct.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --workers 1 --seed 0 --fp 0 --output json
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
dive check conformance/corpus/product-receivers/specifications/BaseValidIntervals/BaseValidIntervals.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --workers 1 --seed 0 --fp 0 --output json
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
dive check conformance/corpus/product-operations/specifications/OrdinaryProductCardinality/OrdinaryProductCardinality.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

`IsFiniteSet` stops at the first empty Product axis or function-set domain in
source order, as TLC does, in both profiles: `IsFiniteSet({} \X x)` is `TRUE`
without asking whether `x` is a set. Record fields are different, because TLC
checks them in the order it first stored the field names. With `--compat=tlc`
Core follows that order, so `IsFiniteSet([zephyrAxis : {}, amberAxis : x] \X {x})`
is `TRUE` when `zephyrAxis` was met first. In the default profile fields are in
canonical order, so every field must be a proved set before an empty field
decides. A field such as `0` refuses with "Cardinality/IsFiniteSet of a Product
with an unproved factor shape or size", and `IsFiniteSet([a : {}, b : 0])` is
never reported `TRUE`. All constructor operands are still evaluated before the
method call; an earlier invalid factor still errors, and an unproved provider
finiteness method still refuses. These checks preserve raw axes and the original Product receiver for
later demands. The `reference-finiteness-demand` and
`reference-finiteness-token-order` corpora record these controls.

This state-dependent example exercises every override on Core:

```sh
dive check crates/dive-api/tests/fixtures/core-graphs/GraphsStep.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --workers 4 --seed 0 --fp 0 --output json
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
dive check crates/dive-api/tests/fixtures/op-literal/specs/OlBagOfAll/OlBagOfAll.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

## Sequence replacement on the core

`SequencesExt!ReplaceFirstSubSeq(r, s, t)` and `ReplaceAllSubSeqs(r, s, t)`
run on Core on strings and on pure, materialized sequences whose
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
materialized outputs are bounded at 2^24 elements. Core refuses the remaining
cases by name, including effectful sequence arguments, whose TLC reevaluation
order is unproved.

This checked fixture needs the CommunityModules source path and completes with
success, 1 distinct state, 2 generated states, depth 1 and exit 0:

```sh
dive check crates/dive-api/tests/fixtures/core-replace/SeqReplacePure.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --seed 0 --fp 0 --workers 4 --output json
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
object and set order
([canonical CHOOSE and order](../tlc-differences.md#d-006-choose-does-not-depend-on-unrelated-text-in-the-module)). JSON strings can affect a specification's
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
independent validation reports the runtime-name refusal; that
word earns no agreement. Conversion support does not remove this limitation.

### Host inputs and explicit grants

`dive check --allow-io=env,files,exec` grants named categories. Both profiles
start with no grants, including `--compat=tlc` ([host access](../tlc-differences.md#d-017-host-access-requires-explicit-grants)). Denial happens before
native host callbacks. A grant supplies permission;
unsupported operators still refuse.

In the default profile, a denied read in a constant definition's startup
evaluation refuses the model only when checking can reach that definition
(from the configuration's roles, assumptions or bindings) or its evaluation
could leave an effect, such as output, a random draw, a register or a file. A
definition such as `PN == CHOOSE s \in 1..10 : ToString(s) = IOEnv.PN` that
nothing checked uses does not need `--allow-io=env`. `--compat=tlc` still
refuses it: TLC's evaluation creates strings that change TLC's later value
order.

Core does not implement external inputs. A specification that reaches
`IOUtils!IOEnv` or `IOUtils!Deserialize` refuses without a grant, naming the
`--allow-io` category it needs; with the grant, Core refuses the run by name
("external inputs ... are not implemented on Core", exit 3) before any host
operation. `dive validate-trace --inputs RUN.json` still replays the
`external_inputs` records of an existing report offline, with exact
operation, path and order checks and no specification I/O.

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
