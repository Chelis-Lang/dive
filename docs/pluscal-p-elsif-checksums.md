# Procedure checksums with P elsif

Full in-memory editing can derive TLC's algorithm header CRC for a bounded P
`elsif` chain containing calls and returns in one disjoint, explicitly labelled
sequential procedure. The paired C syntax uses nested `if` statements. This
extends [nested procedure checksums](pluscal-nested-procedure-checksums.md);
it does not write a file or prove the translated algorithm correct.

Save this runnable example as `ElsifProcedureChecksum_P.tla`:

```tla
---- MODULE ElsifProcedureChecksum_P ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=1;
procedure Put(z=0,a=0)
begin P:x:=a+z; R:return; end procedure;
begin L:if x=0 then skip;
        elsif x=1 then call Put(1,2); goto E;
        else skip; end if;
E:skip; end algorithm; *)
====
```

The equivalent C algorithm comment is:

```tla
(* --algorithm A { variable x=1;
procedure Put(z=0,a=0) { P:x:=a+z; R:return; };
{ L:if (x=0) { skip; }
    else { if (x=1) { call Put(1,2); goto E; } else { skip; }; };
  E:skip; }
} *)
```

```sh
dive translate ElsifProcedureChecksum_P.tla --rewrite --output json
dive translate ElsifProcedureChecksum_P.tla --rewrite --compat=tlc --output json
cargo run -p dive-api --example pluscal_p_elsif_checksum -- ElsifProcedureChecksum_P.tla
```

An admitted report contains `result: replacement`, replacement text in
`replacement.module_source`, verified algorithm/body CRC derivation, and
`model_proved: false`, `exhaustive: false`. Existing divergent bodies/headers
require their respective explicit replacement grants. `--checksums tla` requests
body-only editing; the Cargo example accepts `--body-only`. In TLC profile this
explicit deviation is reported as `tlc+overrides`.

The Cargo-owned example reads the file and calls this public API:

```rust
use dive_api::{RewriteRequest, rewrite_source};
let source = std::fs::read_to_string("ElsifProcedureChecksum_P.tla")?;
let report = rewrite_source("ElsifProcedureChecksum_P.tla", &source, &RewriteRequest::default());
assert!(report.accepted());
println!("{}", report.to_json());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Calls, returns and call-goto transfers may occur in the first, middle or final
branch, including bounded nested If/With/Either/While contexts. The reference's
virtual Else subtree owns an appended continuation update after that whole
subtree. Source/header ordering and logical-token-line TRUE loop behavior remain
observable; formatting or comment changes are not silently normalized away.

That earlier source37 baseline refused multiple-procedure Full checksums.
The later [disjoint procedure guide](pluscal-multiple-procedure-checksums.md)
lifts proved main-only calls into distinct sequential owners. Recursive,
process-owned and scoped-renaming procedure Full checksums remain unsupported. Unproved multiline procedure generation may still
refuse even when the raw header AST and CRC match. Existing bounds are unchanged:
source bytes/tokens/nodes/depth, action count, conservative expansion reservations
and output bytes. A resource refusal emits no admitted replacement. Fair-header
CRC-only equality retains its documented weak source-change detection; a matching
CRC is not an integrity certificate.

A generated model can be checked separately with `INIT Init` and `NEXT Next` in
its config. Finite count/worker/store evidence is distinct from witness validity.
Saved-frame strict replay remains #124: a named Unsupported result is not a
validated counterexample. Canonical-profile replay is separately labelled and
cannot earn TLC-mode validation credit.

The complete evidence selection is 32 clean C/P rows, paired with the unchanged
180 procedure rows and 12 loop studies. Parsed, name-fixed, stored-header and
separate exploded stages are retained. Public whole-module CST/SANY/Validator,
finite models, corrupt headers and forged reports have independent campaigns;
no full PlusCal or API-family certification follows. See
[ADR 0066](decisions/0066-p-elsif-shared-header-checksums.md) and
`crates/dive-pluscal/p-elsif-checksum-acceptance/` for exact source-bound evidence.

Final source37 campaigns closed: all32 new raw stored-header pairs and64 public
profile artifacts match, while all original180 and12 loop dispositions stay
unchanged. Separately declared4 branch-boundary and8 reachable-branch variants
bring the source-qualified total to236 raw observations and472 public profile
rows. The144 finite models match1728 width/store/profile settings;720 independent
replays yield72 canonical valid controls and no strict saved-frame valid witness.
20 executed CLI/Cargo/Rust guides,24 CRC controls and28 forged-report controls
remain distinct from model proof. All51 local-gate stages pass, including1189
tests run with zero skips and unchanged CM1/VTS gates at1/4/8.

Mandatory offline `verify.py` and `verify-delivery.py` read the declared primary
bytes and portable22,105-record source/runtime pack. Original hardcoded replay
path failure, exact external correction and failed draft omission-control
verification stay archived. Post-gate verifier/guide packaging is explicitly
qualified; no runtime or compiled source is retagged. F-034/F-035 stay partial.
