# LOCAL and function definitions in PlusCal

Both profiles can translate bounded global `LOCAL` helpers and TLA function
definitions inside `define`. Source text, domains, helper visibility and state
lookup are preserved. `--compat=tlc` selects the same supported translation and
checksum behavior; it does not authorize host I/O or unproved scopes.

Save the C example as `Forms_C_local_calls_function.tla`:

```tla
---- MODULE Forms_C_local_calls_function ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}]==a+1 LOCAL G(a)==F[a]
 }
{ L:y:=G(1); E:skip; }
}
*)
====
```

The matching P example is `Forms_P_local_calls_function.tla`:

```tla
---- MODULE Forms_P_local_calls_function ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==a+1 LOCAL G(a)==F[a]
 end define;
begin L:y:=G(1); E:skip;
end algorithm;
*)
====
```

```sh
cargo run -p dive-cli --release -- translate Forms_C_local_calls_function.tla --rewrite --output json
cargo run -p dive-cli --release -- translate Forms_P_local_calls_function.tla --rewrite --compat=tlc --output json
```

Successful reports contain `accepted: true`, `result: "replacement"`, the
replacement module and verified algorithm/body CRC derivations. `model_proved`
and `exhaustive` remain false. Generated `F` is a function indexed by `[a]`;
`LOCAL G` remains local to the generated module. Public admission rejects an
invalid enclosing module and never returns its replacement as a success.

The Cargo-owned Rust example uses the same API:

```rust
use dive_api::{RewriteRequest, rewrite_source};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let file = arguments
        .next()
        .ok_or("usage: pluscal_definition_forms SOURCE [tlc]")?;
    let source = std::fs::read_to_string(&file)?;
    let mut request = RewriteRequest::default();
    request.translation.compat_tlc = arguments.next().as_deref() == Some("tlc");
    let report = rewrite_source(&file, &source, &request);
    println!("{}", serde_json::to_string(&report.to_json())?);
    if !report.accepted() {
        return Err("DEFINE source did not yield an admitted artifact".into());
    }
    Ok(())
}
```

```sh
cargo run -p dive-api --release --example pluscal_definition_forms -- Forms_C_local_calls_function.tla
cargo run -p dive-api --release --example pluscal_definition_forms -- Forms_P_local_calls_function.tla tlc
```

For `LOCAL F == x`, `x := 1; y := F` reads the pre-state value of `x`.
Likewise `F[a \in 0..x] == a+1` uses the pre-state domain. The translator does
not inline helpers or replace their variables after an earlier assignment.
Multiple and tuple function bounds use the original declaration syntax.
A generated artifact is not a claim that its domains are finite or executable;
checking a model is a separate request with its own unsupported outcomes.

Action/temporal expressions in either domains or bodies, recursive helper
cycles, symbolic/INSTANCE/RECURSIVE declarations, procedure/DEFINE scopes,
generated/global-name conflicts and unproved renaming remain named
`DIVE-P0090` refusals. The dependency proof conservatively treats formal and
field spellings as possible dependencies. Existing source, output, node and
dependency-work limits remain unchanged; a bound returns `DIVE-P0040`, without
partial artifact or checksum credit. Process-local references are not made
visible by moving their declarations ahead of DEFINE helpers.

[ADR 0056](decisions/0056-pluscal-local-function-definitions.md) and the
[retained reference selection](../crates/dive-pluscal/define-forms-acceptance/README.md)
separate raw post-translation AST/CRC, public CST/SANY/Validator and finite
model/witness evidence. Invalid primary artifacts and the original fixture
filename failures remain uncredited. Strict witness replay under #124 remains
separately qualified; canonical replay cannot count as TLC-mode agreement.
D-020's fair-header limitation remains reference-equivalent: a matching CRC is
not source identity, an integrity certificate or a model proof. Formatting,
filesystem writes and broader scopes remain separate boundaries.


The completed retained selection has 234 raw translator rows: 152 exact AST/CRC
matches, 28 invalid primary artifacts without credit, 52 named refusals and two
primary rejections. Across both profiles, all 468 public rows are accounted for:
304 admitted artifacts with exact whole-module CST, SANY and configured and
unconfigured Validator controls; 36 named refusals; 124 invalid-primary rows
without credit; and four rejections. No row disagrees within that selection.

Separate finite fixtures cover 40 models at 480 profile/worker/store settings,
20 independently valid strict witnesses and 80 rejected witness mutants. Eight
pre-state fixtures run at 96 settings, and six later-state fixtures at 72 settings
also match counts and explicit state invariants. These demonstrate the selected
finite models, not universal finite domains or bounded evaluation cost for every
parsed function definition. Twenty-four import controls verify LOCAL visibility,
exported functions and unknown-name rejection. The CLI/Cargo guides, twelve CRC
controls, and four genuine plus 24 forged report controls also pass.

The corrected executable producer was uncommitted and frozen during those runs;
its exact source archives and binary digests are retained. An earlier examples-only
build left a stale CLI executable: its failed and complete campaigns remain
uncredited, rather than relabelled as current-source conformance. The complete
33-stage gate passes 1,099 tests with zero skips; all 21 CM1 and 16 ValidationTestSuite
smoke models pass at one, four and eight workers. The portable delivery verifier
always reads back all 2,261 frozen input archives and 16,611 runtime records,
including the earlier failed epochs. A later Git delivery does not change the
historical producer identity. Both parity families remain partial.
