# Configuration-owned snapshot observations (#197 / ADR 0057)

The pinned jar SHA-256 is `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`
(source `94d0c5070852b37b801bb8998c436383ab7ee37b`). `SnapshotProbe.java`
constructs actual `FastTool`, reads the pre-constant `Defns` snapshot without
mutation and evaluates the actual selected body with empty states. It does not
supply a fake hook or invoke ordinary model checking/scheduling.

`capture.json`, `selection.json`, `primary.tar.gz` and `models/` retain all 26
original rows. `precedence/` retains all 12 additive startup/name/role rows, with
the same Java observer and its own compile/raw/source archive. No original row
was replaced. The historical producer scripts retain exact machine commands;
reproduction requires supplying the pinned JRE/javac and verified jar in an
isolated directory rather than editing those scripts or archives.

Run the independent byte/event/semantic readback from the repository root:

```sh
python3 -I scripts/check-periodic-snapshot.py
```

The guard checks original selection and source bytes, every raw artifact hash,
complete event lists, configured name versus selected definition, original body
identity and actual errors. Missing/arity failures are 2229/2228; a qualified
configuration role is ConfigFileException 5006; constant roles produce actual
ClassCastException with no invented EC. Unused startup markers precede role
errors. The additive controls preserve earlier constants/init/next/constraints
and reward precedence, periodic-before-VIEW/POSSIBLE, symbolic/quantified startup,
noncached state witnesses and replacements erasing those witnesses.

The bounded native factory independently attempts every original/additive row.
Pure original selection/value cases and original missing/arity classes are
asserted individually. Host/startup work, state/action/temporal hooks, configured
constant-definition captures, substitution scopes and Java exception boundaries
remain named refusals. Reward support remains unimplemented: its row receives no
binding-error parity credit. Small subset/quantifier startup observations remain
TLC positives but native pre-effect refusals. Native outcomes retain output and
configured/selected identity; none certifies ordinary scheduling or a model.

Fresh configured-candidate controls use actual pool widths 1/3/4/8 and separate
fresh binding for independent word, in-memory and JSON replay. Wrong property,
Init, actual closing Next and malformed ending mutants are retained separately.
FALSE-hook diagnostics and actual ordinary checker/exposed capture-model refusal
controls preserve the distinction between a selected hook and an ordinary model
proof. Earlier mistaken fixture expectations and the semantic filename rejection
are retained under development directories, never reclassified as agreements.

`history-before-snapshot` preserves all prior current matrices and the exact
source bytes each claimed (including older archived forwarding/material epochs).
Current integrity maps are refreshed separately; their source hashes do not
retag the original native or primary campaigns.

[User API and executable Cargo example](../../../docs/configured-periodic-snapshot.md)
qualifies source/startup/capture policy limits, typed errors, worker evidence and
remaining ordinary schedule contracts. F-023 remains missing and audit-required.
