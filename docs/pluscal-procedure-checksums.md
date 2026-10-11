# Sequential procedure checksums


Scope update: the later [recursive sequential header contract](pluscal-recursive-procedure-checksums.md)
adds proved bounded self/mutual procedure graphs and tail ownership. Recursive
refusals and counts below describe this guide's original frozen producer; they
are retained history, not the current recursive support boundary. Broader
process/scoped Full contexts and strict #124 saved-frame replay remain pending.

`dive translate FILE --rewrite --output json` can now produce a bounded Full
checksum replacement for one disjoint, explicitly labelled sequential
procedure. Both C and P syntax are supported. The command returns the edited
module as JSON; it does not write the source file. `--compat=tlc` selects the
same supported AST/checksum behavior. `--checksums tla` retains the separate
body-only mode and does not verify an algorithm CRC. A TLC-profile body-only
request is reported as `tlc+overrides`, rather than claiming pure TLC behavior.

Save this P example as `ProcedureChecksum_P.tla`:

```tla
---- MODULE ProcedureChecksum_P ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P:x:=a; R:return;
end procedure;
begin
L:call Put(2); E:skip;
end algorithm; *)
====
```

The corresponding C form is `ProcedureChecksum_C.tla`:

```tla
---- MODULE ProcedureChecksum_C ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
====
```

Run either with:

```sh
dive translate ProcedureChecksum_P.tla --rewrite --output json
dive translate ProcedureChecksum_C.tla --rewrite --compat=tlc --output json
```

An admitted report has `result: "replacement"`, `accepted: true`,
`algorithm_checksum_verified: true` and `body_checksum_verified: true`.
`replacement.module_source` contains the translated module and header. These
fields establish CRC derivation and enclosing-module admission, never a model
proof: `model_proved` and `exhaustive` remain false. A preexisting modified body
or source/header CRC mismatch is refused unless explicitly replacing it with
`--replace-modified`. The original source is not overwritten.

The runnable Cargo example uses the public Rust API:

```sh
cargo run -p dive-api --example pluscal_procedure_checksum -- ProcedureChecksum_P.tla
```

```rust
//! Cargo-owned executable counterpart of the bounded sequential procedure checksum guide.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: pluscal_procedure_checksum FILE [--body-only] [--compat=tlc]")?;
    let mut request = RewriteRequest::default();
    for flag in args {
        match flag.as_str() {
            "--body-only" => request.editing.checksums = ChecksumPolicy::TranslationOnly,
            "--compat=tlc" => request.translation.compat_tlc = true,
            _ => return Err("unknown example option".into()),
        }
    }
    let source = std::fs::read_to_string(&path)?;
    let report = rewrite_source(&path, &source, &request);
    println!("{}", report.to_json());
    if !report.accepted() {
        return Err("rewrite rejected or explicitly unsupported; inspect the report".into());
    }
    Ok(())
}
```

The supported checksum scope includes equality-initialized parameters and
locals (including defaults), ordinary calls, repeated main calls, explicit
call followed by goto, returns and admitted pure control flow. Names and labels
must remain disjoint under the existing translator admission. A procedure action
may contain at most one sequential assignment statement; a simultaneous
assignment may still assign several variables.

Full serialization refuses recursion, multiple procedures, process-owned
procedures, DEFINE/macro combinations, automatic procedure labels, label
modifiers, unproved nested transfer shapes and renaming.
The [nested procedure guide](pluscal-nested-procedure-checksums.md) describes the
proved If/With/Either/While continuation. Some of these
already work in raw translation or body-only editing; those stages are separate
from Full checksum support. No unproved shape receives an invented CRC.

Pinned reference generation retains top-level source calls but mutates shared
nested branch vectors while expanding transfers. The six C/P If/With/Either
call controls were initially retained as named checksum refusals; the later
ADR 0061 continuation now matches their exact original-header bytes. Two original
`local_update` controls are primary parse errors (missing label), while the
existing raw native lowerer admits them; this known raw-stage gap is not counted
as matching rejection or artifact success.

The algorithm checksum is CRC32 over the original parsed/name-fixed AST after
translation, which is not the separately expanded execution AST. The reference
`--fair algorithm` behavior still hashes only `fair` (D-020); matching that CRC
cannot establish source identity. Full reports expose `fair_header_crc_only`.
Disk writes, exact formatting/maps, full scoped renaming and broader checksum
shapes remain pending.

Translation text, parser nodes/depth, actions and serialization bytes retain
existing global bounds; exceeding them returns a typed resource diagnostic
without a replacement or partial CRC. Independent finite checking and trace
validation are separate. The strict #124 encounter-order replay guard remains:
uncertain saved-frame/record-string histories yield a named pending witness,
and a separately labelled canonical replay never earns TLC-profile witness
credit.

See [the declared 132-row study](../crates/dive-pluscal/procedure-checksum-acceptance/README.md)
and [ADR 0058](decisions/0058-pluscal-procedure-checksums.md) for stage identities,
reference bytes, all refusals and retained failed observations.
