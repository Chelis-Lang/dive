# PlusCal DEFINE helpers

`dive translate` can generate bounded global DEFINE helpers in both current
PlusCal syntaxes. `--rewrite` also computes the pinned post-translation AST and
Full checksums for admitted shapes. It returns replacement text in JSON; it does
not write the source file or prove the model.

Save this C-syntax example as `DefineGuide_C.tla`:

```tla
---- MODULE DefineGuide_C ----
EXTENDS Integers
(*
--algorithm A {
 variable x=0;
 define { Step(a) == a+1 }
 { L:x:=Step(x); E:skip; }
}
*)
====
```

The equivalent P syntax, saved as `DefineGuide_P.tla`, is:

```tla
---- MODULE DefineGuide_P ----
EXTENDS Integers
(*
--algorithm A
 variable x=0;
 define Step(a) == a+1 end define;
 begin L:x:=Step(x); E:skip; end algorithm;
*)
====
```

```sh
cargo run -p dive-cli --release -- translate DefineGuide_C.tla --rewrite --output json
cargo run -p dive-cli --release -- translate DefineGuide_P.tla --rewrite --compat=tlc --output json
```

Both reports return `result: "replacement"`, `accepted: true`,
`algorithm_checksum_verified: true`, and `body_checksum_verified: true`.
`model_proved` and `exhaustive` stay false. Read the replacement's `module_source`
and generate a separate configuration/check request to model-check it.
The generation-only command without `--rewrite` returns an admitted `artifact`.
The `--compat=tlc` counterpart uses the same supported translation semantics.

The Cargo-owned Rust example in `crates/dive-api/examples/pluscal_definitions.rs`
executes the same API contract:

```rust
use dive_api::{RewriteRequest, rewrite_source};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let file = arguments
        .next()
        .ok_or("usage: pluscal_definitions SOURCE [tlc]")?;
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
cargo run -p dive-api --release --example pluscal_definitions -- DefineGuide_C.tla
cargo run -p dive-api --release --example pluscal_definitions -- DefineGuide_P.tla tlc
```

Global variables and `pc` are declared before helpers; process locals are declared
after helpers. A helper cannot obtain visibility of a process-local variable by
moving that declaration earlier. The generated enclosing module must pass syntax
and semantics, or the public report returns a rejection with no replacement.
Forward references and malformed declarations in the retained primary outputs
likewise earn no artifact credit.

Helpers keep TLA state lookup. For `F == x`, followed by `x := 1; y := F`, the
reference and dive emit `x' = 1 /\ y' = F`; `F` reads the pre-state `x`.
They do not inline it as the newly assigned value. Parameters, records, finite
function expressions, CASE, comments, LF/CRLF and tabs have separate captured
scope/layout controls. Macro expansion remains limited by its own documented
substitution contracts.

Bounded pure global LOCAL/function definitions are now described in
[the function-definition guide](pluscal-definition-forms.md). Recursive operators,
`RECURSIVE` declarations, symbolic or INSTANCE declarations, action/temporal helpers, procedure/DEFINE scopes,
renaming conflicts and unproved cycles remain named `DIVE-P0090` refusals.
The dependency proof is conservative about formal/field spellings. Byte/node/
action limits are unchanged; hitting a bound returns `DIVE-P0040` and no partial
artifact. Neither profile permits arbitrary host I/O implicitly.

The [reference captures](../crates/dive-pluscal/define-acceptance/primary/manifest.json)
and [ADR 0053](decisions/0053-pluscal-global-definitions.md) separate raw AST/CRC,
public generated-module/CST/SANY/Validator controls and finite-model/witness
checks. The original Echo dependency failure is retained; a second capture loads
its exact pinned local Relation module. Echo's nonliteral process IDs and the
unbounded original FindHighest model are not certified by this increment.
Existing checksum-less headers, fair-marker CRC weakness (D-020), exact pretty
bytes, filesystem writes and translator maps remain separately qualified in
[rewriting](pluscal-rewriting.md). An admitted checksum is not an integrity or
model certificate; strict independent witness gaps under #124 remain explicit.

The [developer evidence record](../crates/dive-pluscal/define-acceptance/README.md) explains the original scoped runtime source, retained qualification gaps, portable raw records and the separate fresh integration gate. These records establish the stated bounded behavior, not whole-language certification.
