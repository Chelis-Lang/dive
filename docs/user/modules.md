# Modules and evaluated operators

```sh
dive analyze docs/user/examples/ModuleExample.tla --output json
dive check docs/user/examples/ModuleExample.tla --compat=tlc --seed 7 --fp 0 --workers 4 --output json
```

This model imports `Naturals`, `Sequences` and `FiniteSets`, then checks an
invariant using arithmetic, `Len` and `Cardinality`. A module can parse and resolve while a particular operator, argument shape or
checking mode is still unsupported, so a clean analysis does not mean the model
will run.

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
name. The library resolved; the refusal is about checking, and it gives no verdict.

The native evaluator implements supported arithmetic, sets, sequences, records,
functions and selected standard/CommunityModules operators. Standard-module
source and TLC's native overrides are distinct: model binding decides which
implementation applies. Integer limits, comparison behavior and enumeration
order follow the resolved [compatibility profile](compatibility.md). An operator listed in the registry may still refuse some arguments or modes.

Core resolves a configured constant such as `C <- Target` through imported
declarations inside `INSTANCE`, including nested instances. Explicit `WITH`
bindings still take precedence. The target keeps its own module and lexical
scope, while state substitutions retain their current or primed meaning.
The configured target's body runs when demanded; ordinary helpers retain their
successful startup values, including Print and RNG results. Checker registers cannot be written during startup: an integer register write
leaves the definition lazy, and a named write stores no value. ASSUMEs
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
still raises division by zero. Values computed successfully at startup stay cached; resource failures and
interruptions propagate and are not retried. This rule is
the same in both profiles. This example succeeds with 2 distinct states:

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
does not inspect unused range factors; a nonempty sequence checks each element for membership in the original
product, so a non-tuple element still raises its error, even when the product
is empty. Both profiles run this
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

Nested `SUBSET` membership can enumerate a literal set, normalizing it before
the first element test. An empty query leaves
the base and independent aliases untouched. Unproved raw receivers and demanded
mixed-kind comparisons remain explicit refusals. The checked example is:

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
a standard module have the same name. The conformance harness pins module libraries to compare dive with TLC; that
does not make every library override native.

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
comparison needed by that diagnostic. The minimal state-dependent example succeeds in either profile:

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
Unknown earlier operands and providers keep their errors or named refusals. Dive does not verify that this error has TLC's error class.

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
evaluation error, including after graph conversion. It exits 75; dive does not verify that the error has TLC's error class. Nested product comparisons, general set elements,
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

The pinned Examples Hanoi `Bits.class` has a native override for exhaustive
checking. Binding
checks the exact SHA-256 recorded in [provenance](../provenance.md#verified-local-class-overrides)
before replacing `Bits!And(x,y,n,m)` with signed 32-bit AND of `x` and `y`.
All four arguments must be TLC 32-bit integers, in both profiles. The class must
sit beside the resolved `Bits.tla`; without it, the TLA+ body applies. A changed,
unreadable or unknown class produces a named `unsupported` result. Configuration
substitution and arity errors retain precedence. The refusal occurs before evaluating constant
definitions, assumptions or configured predicates, so an unlike TLA+ fallback
cannot produce a verdict or error for an unimplemented class. Checkpoints record
the exact class identity, and removing or changing the class rejects recovery
(see [storage and recovery](storage-recovery.md)). Every supported worker count
gives the one-worker result: the same counts, depth, property, error class and
exit code, with an independently validated counterexample. Simulation and DFID
follow their own rules. The earlier one-worker restriction is recorded in
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
status. This fixture calls `BagOfAll` in an `ASSUME`, so it reports
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
materialized outputs are bounded at 2^24 elements. 

This checked fixture needs the CommunityModules source path and completes with
success, 1 distinct state, 2 generated states, depth 1 and exit 0:

```sh
dive check crates/dive-api/tests/fixtures/core-replace/SeqReplacePure.tla --library-path crates/dive-eval/tests/fixtures/community-modules --compat=tlc --seed 0 --fp 0 --workers 4 --output json
```

The Rust API uses the same `CheckRequest.library_paths` and
`check_file(&request)`; see the
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
`ndJsonDeserialize` remain named refusals. The conversion needs no host access.

State-time `ToJson` creates string names. When value or exploration order follows
TLC (`--compat=tlc`), checking reports the existing sequential worker fallback
so those names retain encounter order. The default ordering can use the
requested worker pool. The JSON output is the same either way, and independent trace validation
still refuses traces that depend on string names created after startup.

The consumed `crates/dive-eval/tests/fixtures/json/JsonSupported.tla` and `.cfg`
hold the pinned-oracle example (`ToJson(<<"x", TRUE, <<1,2>>>>)`
returns `"[\"x\",true,[1,2]]"`). Run the fixture with:

```sh
dive check crates/dive-eval/tests/fixtures/json/JsonSupported.tla --config crates/dive-eval/tests/fixtures/json/JsonSupported.cfg --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

Replay accepts JSON strings whose names already exist in the bound model. If
the deciding counterexample needs JSON string names created at run time in the
TLC profile, independent validation refuses it by name.

### Host inputs and explicit grants

`dive check --allow-io=env,files,exec` grants named categories. Both profiles
start with no grants, including `--compat=tlc` ([host access](../tlc-differences.md#d-017-host-access-requires-explicit-grants)). Denial happens before
native host callbacks. A grant supplies permission;
unsupported operators still refuse.

In the default profile, a denied read while evaluating a constant definition at
startup refuses the model only in two cases: checking can reach that definition
from the configuration's roles, assumptions or bindings, or its evaluation
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
and known limitations. 

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
configured evaluation depth remain resource limits. Arguments are evaluated left to right before their shapes are checked, and an
explicit TLA+ definition override takes priority.

The fixture uses a small SVG binding module and reaches an invariant
violation after two states:

```sh
dive check crates/dive-api/tests/fixtures/svg-point/Point.tla --module-path crates/dive-eval/tests/fixtures/svg-point --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

Its expected physical CLI status is 12; `--exit-codes native` explicitly selects
10. Rust `CheckReport::exit_code()` keeps its profile-specific status. Only this
helper is native: SVG element serialization, graph and ring layout, host
process access and animation completion are not implemented.
