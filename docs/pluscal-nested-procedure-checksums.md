# Nested procedure checksums

Full in-memory editing now derives the pinned algorithm header checksum for
proved nested calls and returns in one disjoint, explicitly labelled sequential
procedure. Both C and P syntaxes are supported. This expands the
[top-level procedure contract](pluscal-procedure-checksums.md); it does not
write files or prove the translated model correct.

Use a nested call in a P algorithm:

```tla
---- MODULE NestedProcedureChecksum_P ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
begin P: x:=a; R: return; end procedure;
begin
L: with q=2 do
     if x=0 then call Put(q); goto E; else skip; end if;
   end with;
E: skip;
end algorithm; *)
====
```

The equivalent C body is:

```tla
(* --algorithm A {
variable x=0;
procedure Put(a=0) { P: x:=a; R: return; };
{ L: with (q=2) { if (x=0) { call Put(q); goto E; } else { skip; }; };
  E: skip; }
} *)
```

```sh
dive translate NestedProcedureChecksum_P.tla --rewrite --output json
dive translate NestedProcedureChecksum_P.tla --rewrite --compat=tlc --output json
cargo run -p dive-api --example pluscal_nested_procedure_checksum -- NestedProcedureChecksum_P.tla
```

An admitted report has `result: replacement`, the replacement module in
`replacement.module_source`, verified algorithm/body CRC derivation, and
`model_proved: false`, `exhaustive: false`. Existing modified headers/bodies are
refused unless the corresponding explicit replacement grant is supplied.
`--checksums tla` requests body-only editing; TLC profile then reports
`tlc+overrides`. The Rust example also supports `--body-only`.

The compiled Cargo example uses the same public API:

```rust
use dive_api::{RewriteRequest, rewrite_source};
let request = RewriteRequest::default();
let source = std::fs::read_to_string("NestedProcedureChecksum_P.tla")?;
let report = rewrite_source("NestedProcedureChecksum_P.tla", &source, &request);
assert!(report.accepted());
println!("{}", report.to_json());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Supported shared-header mutations include final nested calls, returns and
call-followed-by-goto in If, With and Either; mixed transfer/nontransfer branches;
empty opposite If branches; nested controls and nested controls inside labelled
While bodies. Direct top-level transfers and direct unlabelled While transfers
retain source header nodes. Argument/local expression slices preserve the
reference logical token lines. Parameter/frame simultaneous assignments use
reference ordering; restores remain separate assignments.

Full still refuses recursive/multiple/process-owned procedures, unproved scoped
renaming, macro/DEFINE combinations, automatic procedure labels, modifiers,
branch-internal labels and nested transfer chains through `elsif`. A procedure
label still admits at most one sequential source assignment. These restrictions
are checksum-stage restrictions: raw translation may support more. No CRC is
invented for an unsupported request, including a fair algorithm.

Existing parser node/depth, normalization work, action and 4 MiB output ceilings
remain. Typed synthetic frame work is charged before allocating assignments;
exhaustion returns Resource without a replacement or partial CRC. Matching a
`--fair algorithm` CRC still means matching the reference hash of `fair`, not
source identity (D-020).

The reference study retains all 180 rows, original failures, reference parser
errors and SANY-invalid outputs. Header byte equality alone cannot admit a
module: public admission separately requires native front-end validation.
Finite models and independent witnesses are separate evidence. Strict TLC
saved-frame/record-string replay can remain named unsupported under #124;
canonical replay is separately qualified and earns no strict TLC witness credit.
See [ADR 0061](decisions/0061-nested-procedure-header-vectors.md) and the
[stage-separated study](../crates/dive-pluscal/nested-procedure-checksum-acceptance/README.md).
