# Full checksums for acyclic procedure calls

The bounded Full editor derives TLC's original-header CRC for globally disjoint,
explicitly labelled sequential procedures that call other procedures without
recursion. Forward and backward declarations, target parameters/locals, ordinary
calls, adjacent call-goto and the already proved If/elsif/With/Either/While transfer
contexts are supported. This extends the [main-only procedure checksum study](pluscal-multiple-procedure-checksums.md).
Editing produces replacement text in memory; it does not write a file or prove a model.

Save this as `AcyclicProcedureChecksum_P.tla`:

```tla
---- MODULE AcyclicProcedureChecksum_P ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
```

The C algorithm comment is:

```tla
(* --algorithm A { variables x=0;
procedure Outer(p=0) { O1:call Inner(p+1); O2:x:=p; O3:return; };
procedure Inner(q=0) { I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
```

```sh
dive translate AcyclicProcedureChecksum_P.tla --rewrite --output json
dive translate AcyclicProcedureChecksum_P.tla --rewrite --compat=tlc --output json
cargo run -p dive-api --example pluscal_acyclic_procedure_checksum -- AcyclicProcedureChecksum_P.tla
```

The Cargo-owned example uses the public API below:

```rust
use dive_api::{RewriteRequest, rewrite_source};
let source = std::fs::read_to_string("AcyclicProcedureChecksum_P.tla")?;
let report = rewrite_source("AcyclicProcedureChecksum_P.tla", &source, &RewriteRequest::default());
assert!(report.accepted());
println!("{}", report.to_json());
# Ok::<(), Box<dyn std::error::Error>>(())
```

An accepted report has `result: replacement`, `replacement.module_source`,
verified algorithm/body CRC derivation, `model_proved: false` and `exhaustive: false`.
`--checksums tla` requests body-only editing; the Rust example accepts `--body-only`.
A compat request with overrides reports `tlc+overrides`. Modified existing bodies
and headers still require explicit replacement grants. Artifact admission is
separate from checksum derivation and checks the enclosing generated TLA module.

Calls save the target procedure's parameter/local slots and continuation; returns
restore the current procedure's own saved slots. All slot/label names must be
globally disjoint in this checksum scope. Actual procedure-body edges determine
acyclic admission. Main calls do not create edges in that graph.

The reference header retains each owner's source call list and appends main's
distinct targets to the last declaration. In the example, the header shows
Outer=[Inner], Inner=[Outer], although runtime calls form no recursive cycle.
Reversing the declarations changes that CRC bookkeeping. Unused procedures still
retain their parsed source calls. This is measured pinned behavior, not a
reachability statement; the runtime frame owner remains explicit.

Self/mutual recursion, process-owned procedure checksums, scoped name rewriting,
macros/definitions in this procedure checksum context, automatic labels,
unproved modifiers and multiline procedure generation remain named Unsupported
results with no successful Full replacement. Raw translation can have a broader
scope. Invalid reference commands and reference parse errors remain their own
uncredited/accounted dispositions. No refusal is counted as complete parity.

The original source/token/node/depth/action/expansion/output ceilings are unchanged.
Graph discovery and all owners share one conservative reservation before owned
expansion allocation. A resource refusal returns no partial successful CRC.
Output also rejects zero/tiny/one-byte-short budgets and accepts the exact byte boundary.

Fair-header-only CRC equality retains the pinned weak detection policy; it does
not certify source identity. Finite model counts and independent witnesses are
separate checks. Strict saved-frame replay remains pending under #124 and can
return named Unsupported; separately labelled canonical replay earns no
TLC-profile validation credit.

The selection contains all prior 98 cases and 32 new C/P cases, declared before
native changes. The clean primary retry has 116 SANY-valid artifacts and 14
reference errors. Original-after-generation header stages and direct exploded
ownership remain distinct. Focused controls require exact stored-header bytes/CRC
for all 32 new cases and the four formerly refused forward/reverse owner cases,
with retained recursive negatives and unchanged aggregate budgets. Complete
public CST/SANY/Validator, model/mutant and runnable guide evidence is recorded
separately as qualification finishes. See [ADR 0073](decisions/0073-acyclic-procedure-header-checksums.md)
and `crates/dive-pluscal/acyclic-procedure-checksum-acceptance/`.
F-034/F-035 remain partial; this scope is not a full translator/API certificate.
