# Native sequential PlusCal procedures

`dive translate FILE.tla --output json` now admits one explicitly labelled
sequential procedure with disjoint global/parameter/local names. Save this
Pascal-style module as `ProcedureExample.tla`:

```tla
---- MODULE ProcedureExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Put(a = 0)
variable b = a + 1;
begin
  Entry: x := b;
  Return: return;
end procedure;
begin
  Start: call Put(2);
  Finish: skip;
end algorithm; *)
====
```

For C syntax, save this as `ProcedureExampleC.tla`:

```tla
---- MODULE ProcedureExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Put(a = 0)
  variable b = a + 1;
  { Entry: x := b; Return: return; };
  { Start: call Put(2); Finish: skip; }
} *)
====
```

The result is `artifact`, `accepted=true`, `exit_code=0` at `artifact-analysis`,
with `model_proved=false` and `exhaustive=false`. The complete validated module
is `module_source`; the generated body is `artifact.tla`. Translation does not
write or update input files. Rust callers use `dive_api::translate_source` or
`translate_file` with `TranslationRequest`; both default and `--compat=tlc`
profiles share this admitted contract. Existing parser/action/output limits
remain effective. `--label` does not admit unproved procedure-label insertion.

A minimal Rust caller, run next to the saved module, is:

```rust
use dive_api::{TranslationRequest, translate_file};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = translate_file(Path::new("ProcedureExample.tla"), &TranslationRequest::default())?;
    assert!(report.accepted());
    assert_eq!(report.to_json()["model_proved"], false);
    println!("{}", report.to_json());
    Ok(())
}
```

The declaration initializes stored parameter/local values for `Init`. Calling
`Put(2)` pushes their old values and the `Finish` continuation on `stack`, sets
`a'=2`, then initializes `b'=a'+1`. The callee starts at `Entry`. Returning
restores `pc`, then locals, then parameters and pops the frame. This restoration
means the saved old values remain observable after returning. Calls with no
parameters and default model-value initialization are supported, as are admitted
branch/loop/parallel-assignment bodies and an immediate `call ...; goto Label`.

A procedure without a final `return` generates `pc'="Error"` on fallthrough,
matching the reference. It does not acquire an implicit return. Such an artifact
can deadlock when checked; translation success never claims a successful model.

Change `call Put(2)` to `call Missing(2)` for a source/generation rejection with
exit 1 and no artifact. A nested/recursive call, multiple procedures, renaming
collision, procedure/process ownership or procedure/macro interaction returns
named `unsupported` with exit 3. Undefined TLA names in a generated module are
rejected at enclosing artifact analysis. An unused invalid procedure still
prevents module admission; its invalid reference output is never credited.

## Evidence and boundaries

`crates/dive-pluscal/procedure-acceptance` retains all 84 original C/P rows and
clean pinned translator/normalization/SANY stages. The active token study has
48 admitted artifacts, 14 matching source rejections, 20 named refusals and two
diagnostic-only invalid-reference dispositions. Four primary translated modules
fail SANY; all remain uncredited. Admitted artifacts require full enclosing-module
operator CST and independent SANY checks, rather than token equality alone.
The 76 curated model controls reproduce clean TLC counts/verdict/depth at
1/4/8 workers in fingerprint/exact stores (456 controls). Forty-six complete
searches agree. Thirty invariant-witness runs remain pending strict validation:
the TLC-profile validator explicitly refuses generated stack concatenation under
the runtime string encounter-order guard (#124). This refusal is preserved in
the original strict report; it earns no fully validated TLC violation credit.

Separately labelled default/canonical replay validates these 30 concrete traces
and rejects 120 corrupted Init/Next/closing/missing-variable controls. It uses
`TraceOptions::default()` (CLI without `--compat=tlc`); it is not switched-profile
TLC certification. Both report profiles remain explicit. A future narrow
namespace-provenance proof must precede strict validation credit; this increment
does not relax the existing guard. Reports and the delivery receipt bind sources,
scripts, binaries and captures.

The initial model-validator selection incorrectly expected a violation at label
`E` in the call-to-Done case, which has no `E`. Primary TLC returned success;
that stopped campaign and its original captures are retained. The corrected
study uses it only as a complete finite model. It earns no violation credit.

Source, depth, action and generated-byte limits are bounded. The normalized-node
quota is global across main/procedure output. Recursive/nested calls, multiple
procedure scopes, source renaming, process-local stacks, procedure fairness,
automatic labels and macro/procedure expansion remain pending. Historical
options, pretty width, rewrite/checksum and mapping-format parity are pending.
F-034/F-035 remain partial; see [ADR 0041](decisions/0041-pluscal-sequential-procedure-calls.md).
