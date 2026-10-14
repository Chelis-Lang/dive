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

Repeat `--library-path DIR` to search before bundled standard modules or
`--module-path DIR` to search after them. These directories contain TLA+ module
sources; accepting a source module does not load arbitrary Java plugins. Read
the resolved files reported by [analysis](front-end.md) when a local module and
a standard module have the same name. Pinned module libraries in the conformance
harness support oracle studies; they do not turn every library override into
native functionality.

`RandomElement` and admitted `Randomization` operations use the run's recorded
seed and mode-specific replay contract. Checker-register operations such as
`TLCGet` and `TLCSet` need runtime context and can change parallel eligibility.
A pure safety run admitting these operators does not mean simulation, temporal
capture, checkpoint recovery or independent trace validation admits them too.
Read each mode's supported/refused combinations.

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

Host environment reads, file operations and process execution remain named
refusals in this checkout; it does not expose the planned category-grant CLI.
Future host access must require explicit category grants even in the TLC profile
(see [compatibility](compatibility.md) and D-017 in the differences log). A grant
will not invent an unimplemented operator. Unsupported operations or missing
host context must not be interpreted as passing the model invariants.

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
