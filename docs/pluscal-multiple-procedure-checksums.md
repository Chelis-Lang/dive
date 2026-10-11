# Full checksums for disjoint sequential procedures

The bounded Full editor can derive the pinned algorithm CRC for multiple explicitly labelled sequential procedures called from main, with disjoint names and parameter/local slots. Calls save the target procedure's slots; returns restore the current procedure's slots. This extends [procedure checksums](pluscal-procedure-checksums.md) and [nested/elsif headers](pluscal-p-elsif-checksums.md). Editing returns replacement text in memory; it does not write a file or prove the generated model.

Save this P-syntax example as `MultipleProcedureChecksum_P.tla`:

```tla
---- MODULE MultipleProcedureChecksum_P ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0;
procedure Put(z=0,a=0)
begin P1:x:=a+z; P2:return; end procedure;
procedure Get(q=0,b=0)
begin G1:x:=b+q; G2:return; end procedure;
begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
====
```

Its C algorithm comment is:

```tla
(* --algorithm A { variable x=0;
procedure Put(z=0,a=0) { P1:x:=a+z; P2:return; };
procedure Get(q=0,b=0) { G1:x:=b+q; G2:return; };
{ L:call Put(1,2); M:call Get(3,4); E:skip; }
} *)
```

```sh
dive translate MultipleProcedureChecksum_P.tla --rewrite --output json
dive translate MultipleProcedureChecksum_P.tla --rewrite --compat=tlc --output json
cargo run -p dive-api --example pluscal_multiple_procedure_checksum -- MultipleProcedureChecksum_P.tla
```

The Cargo-owned example uses this public API:

```rust
use dive_api::{RewriteRequest, rewrite_source};
let source = std::fs::read_to_string("MultipleProcedureChecksum_P.tla")?;
let report = rewrite_source("MultipleProcedureChecksum_P.tla", &source, &RewriteRequest::default());
assert!(report.accepted());
println!("{}", report.to_json());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Accepted output has `result: replacement`, `replacement.module_source`, verified algorithm/body CRC derivation and `model_proved: false`, `exhaustive: false`. `--checksums tla` requests body-only editing; the Rust example accepts `--body-only`, reported as `tlc+overrides` when combined with compat mode. Existing divergent bodies and headers require their explicit replacement grants. Replacement/checksum acceptance still requires separate TLA artifact admission.

The source/header lists calls on the last declared procedure in main encounter order, even when that procedure was never called. This matches the pinned parser's shared vector; it is not a reachability claim. Reversing declaration or call order may change that header CRC. Ordinary calls, call-goto, target locals and owner-specific returns use the existing proved If/elsif/With/Either/While contexts.

Procedure-to-procedure calls, recursion, process-owned procedures, scoped name rewriting, macros/definitions in this checksum scope, automatic labels and unproved label modifiers remain named unsupported results with no admitted replacement. Raw translation can support more than Full checksum derivation. All source/token/node/depth/action/expansion/output bounds remain unchanged and aggregate across owners. A resource refusal carries no successful checksum or artifact claim.

Fair-header-only CRC equality remains the pinned weak detection policy, not source identity or an integrity certificate. A generated model may be checked independently with `INIT Init` and `NEXT Next`; finite count/width/store evidence is distinct from witness validity. Strict saved-frame replay remains #124 and may return named Unsupported. Separately labelled canonical replay never earns TLC-profile validation credit.

The declared study retains all 98 source/error dispositions and original refusal evidence, plus runnable guide controls, primary original-header bytes, separate exploded stages, CST/SANY/Validator admission and model/mutant controls. See [ADR 0069](decisions/0069-disjoint-procedure-checksum-ownership.md) and `crates/dive-pluscal/multiple-procedure-checksum-acceptance/`. F-034/F-035 remain partial; no full translator or API certificate follows.


Current focused controls retain62 exact original-header pairs and2 original
grammar errors, plus exact corrected declarations. The shared budget test fits48
alternating calls into two owners in source grammar, then requires a named
Normalization resource error before CRC emission. Serialized output limits
reject zero/tiny/one-byte-short budgets and accept the exact byte boundary.
Full final source gates and runtime campaigns remain required before delivery;
the first uncommitted producer and invalid-command driver failures retain their
own identities. The invalid `-fair` spelling is not a supported option: clean
pinned CLI exit255 and native exit2 earn no language-checksum or exit-equivalence
credit. Use the documented typed fairness options instead.
