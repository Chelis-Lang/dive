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

Host environment reads, file operations and process execution remain named
refusals in this checkout; it does not expose the planned category-grant CLI.
Future host access must require explicit category grants even in the TLC profile
(see [compatibility](compatibility.md) and D-017 in the differences log). A grant
will not invent an unimplemented operator. Unsupported operations or missing
host context must not be interpreted as passing the model invariants.

For coverage details, the [parity ledger](../tlc-parity.md) separates module
sources, native override registrations and semantic feature families. The
[maintained TLC differences log](../tlc-differences.md) describes approved changes
and known limitations. These inventories and curated examples do not certify
all standard-module or CommunityModules behavior.
