# Full checksums for recursive sequential procedures

The bounded editor derives the pinned TLC algorithm-header CRC for explicitly
labelled sequential procedures with globally disjoint names, including self and
mutual recursion. It serializes the finite source AST; it does not unroll calls,
prove termination or write files. Raw checksum derivation, admitting the generated
TLA module and checking that model are separate operations.

Save this as `RecursiveHeader_P_self_base.tla`:

```tla
---- MODULE RecursiveHeader_P_self_base ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:if n>0 then call Recur(n-1); else x:=x+1; end if;R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
```

The equivalent C algorithm comment, used in the paired C guide control, is:

```tla
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:if (n>0) { call Recur(n-1); } else { x:=x+1; };R2:return; }; { L:call Recur(2); E:skip; } } *)
```

```sh
dive translate RecursiveHeader_P_self_base.tla --rewrite --output json
dive translate RecursiveHeader_P_self_base.tla --rewrite --compat=tlc --output json
cargo run -p dive-api --example pluscal_recursive_procedure_checksum -- RecursiveHeader_P_self_base.tla
```

The Cargo-owned executable calls the public API:

```rust
use dive_api::{RewriteRequest, rewrite_source};
let source = std::fs::read_to_string("RecursiveHeader_P_self_base.tla")?;
let report = rewrite_source("RecursiveHeader_P_self_base.tla", &source, &RewriteRequest::default());
assert!(report.accepted());
println!("{}", report.to_json());
# Ok::<(), Box<dyn std::error::Error>>(())
```

An admitted report has `result: replacement`, verified algorithm/body CRC
derivation and `replacement.module_source`. It also has `model_proved: false`
and `exhaustive: false`. The paired reference header CRC is `72442bcc`; this is
reference derivation equality, not a source-integrity or termination certificate.
`--checksums tla` selects body-only editing; the Cargo example accepts
`--body-only`. Overrides in TLC profile report `tlc+overrides`.

Ordinary recursive calls save target parameter/local slots and continuation.
Returns restore the current owner. An adjacent self call/return keeps the existing
frame, initializes parameters/locals and jumps to entry. Cross-owner tail calls
restore caller locals and replace the frame head first. The header retains parsed
CallReturn nodes at top level while shared nested If/elsif/With/Either/While
vectors retain the pinned exploded mutation. Source-owned calls and the LAST
header's shared main alias are distinct; unused recursive owners still serialize.

Both profiles retain the same source-backed checksum behavior. Process-owned
procedure Full checksums, scoped renaming, definitions/macros in this procedure
scope, automatic labels, unproved modifiers and multiline procedure generation
remain named refusals with no successful Full replacement. All original resource
ceilings remain; owners share conservative expansion reservations before owned
allocation. There is no per-procedure quota multiplication. Zero/tiny/one-byte-short
output limits refuse; the exact serialized-byte boundary is tested.

The clean reference generates invalid TLA for a zero-parameter self-tail call.
Its raw AST/CRC remains diagnostic evidence, while the public editor rejects it
and emits no successful replacement. Nonterminating recursion can have a valid
translation/header; that is not a completed model search. Such inputs are excluded
from finite-completion controls and keep separate bounded DFID unknown/refusal
evidence. `dive dfid Generated.tla --max-level 4 --output json` can report
`incomplete` or a named `unsupported`, never a proof derived from the CRC.

Strict saved-frame witness replay still has the #124 namespace limitation. Named
Unsupported and canonical-only Valid are recorded separately; canonical replay
never counts as strict TLC-profile agreement. See the [recursive execution
contracts](pluscal-recursive-procedures.md) and [acyclic predecessor study](pluscal-acyclic-procedure-checksums.md).

The complete declared 162-input selection preserves the previous 130, adds 32
recursive shapes and retains every error/invalid artifact. Its clean primary and
separate direct exploded stages are checked independently. Before native changes,
104 raw headers matched and 50 were named refusals. The first native attempt
revealed the shared self-tail mutation gap; that failed source/output is preserved.
A separate eight-case study adds self/cross-owner tail locals, simultaneous
arguments and reversed declarations. Its first three malformed C local declarations
and all original primary errors remain retained before the corrected clean captures. The parsed 16-owner/128-node expansion
negative fails if shared enforcement is deliberately disabled and passes after
restoring the exact production bytes.
Focused controls now require all 32 new exact header bytes/CRC, all eight
field-order controls, four previously
refused old recursion controls, original acyclic bytes, actual arity failures and
shared expansion/output limits. Broader public and finite-model qualification is
reported only after the source-frozen campaign closes.

See [ADR 0077](decisions/0077-recursive-procedure-header-checksums.md),
[the complete retained selection](../crates/dive-pluscal/recursive-procedure-checksum-acceptance/README.md)
and [D-008's artifact/report qualification](tlc-differences.md).

The frozen recursive producer passed all 54 local commands, including 1,203
nextest cases with zero skipped. Its fresh 170-row raw selection has 146 exact
AST/CRC matches, 14 named refusals, six accounted primary errors, two invalid
CLI commands and two SANY-invalid primary artifacts without admission credit.
The paired 340 public routes retain 292 exact CST/SANY/Validator admissions,
28 named refusals, 12 primary-error dispositions, four invalid commands and
four invalid primary artifacts. All older regression selections remain visible.

The original 22 runtime invocations closed with four missing-script-path
failures (stages 10–13); those invocations have no case credit. Exact archived
canonical drivers were restored into a separate path-repair epoch, which freshly
ran the affected 12 sources and 24 public routes on unchanged binaries and
source. Commands, logs and both dispositions are retained rather than replaced.
The finite controls cover 98 sources, 196 complete/violation runs and 2,352
profile/worker/store settings. The 980 independent replay controls have 98
canonical Valid, 392 canonical Invalid, 294 strict Unsupported and 196 strict
Invalid: **zero strict saved-frame Valid**. Twenty executed CLI/Cargo guide
routes, 24 corruption controls, 28 schema-forgery controls, eight bounded
nontermination controls, CM1 21 and VTS 16 at widths 1/4/8 are separate evidence.
None converts artifact derivation into a model proof or full-family certificate.

The selected frozen archive covers 5,264 inputs, including 451 standard compiled
Rust inputs and 65 literal include targets. It is not a hermetic dependency,
environment or dynamic-path closure. A separate post-closure supplement contains
786 omitted older reference files (with frozen Git/prior archive identity),
26 declared runtime tools/drivers and two exact restored-driver archives. It
explicitly records original-command versus repair-only use; those files are
never retroactively added to the original freeze or credited as native parity.
The always-on portable reader verifies raw records without a local evidence
cache. Optional binary readback remains distinct from source/gate verification.
