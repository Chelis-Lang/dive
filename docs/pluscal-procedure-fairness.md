# Fair processes calling procedures

This bounded native increment translates weak and strong fairness for finite
processes calling procedures. It generates a TLA+ `Spec`; it does not search the
liveness graph or prove termination. Both source grammars use a modifier after
the label's colon: `Entry: +`, not `Entry+:`.

Save the P-style source as `FairCall.tla`:

```tla
---- MODULE FairCall ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Add(a = 0)
begin
  Entry: + x := x + a;
  Finish: return;
end procedure;
fair process One = 1
begin
  Start: call Add(1);
  DoneOne: skip;
end process;
end algorithm; *)
====
```

Equivalent C syntax:

```tla
---- MODULE FairCallC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Add(a = 0) { Entry: + x := x + a; Finish: return; };
  fair process (One = 1) { Start: call Add(1); DoneOne: skip; }
} *)
====
```

Run `dive translate FairCall.tla --compat=tlc --output json`. The admitted
`module_source` includes `WF_vars(One)`, `WF_vars(Add(1))` and
`SF_vars(Entry(1))`. With multiple callers, each fair caller receives clauses for
its own ID, while procedure actions and storage declarations remain shared.
A membership caller uses `self` under its process-domain quantifier.

Use `fair + process` for strong fairness. A weak caller's `+` label adds strong
fairness for that labelled action; a strong caller already uses strong fairness.
A `-` label excludes that program-counter location from the owning body or
procedure's fairness clause. Multiple minus labels form a set. These modifiers
never change generated action ownership or execute a procedure at translation
time.

The translator options keep their pinned meaning:

| CLI option | Effect |
| --- | --- |
| No option | Preserve declared process and algorithm fairness. |
| `-wf` | Supply weak fairness to otherwise unfair processes. |
| `-sf` | Supply strong fairness to otherwise unfair processes. |
| `-nof` | Suppress process/procedure and algorithm fairness in process algorithms. |
| `-wfNext` | Add `WF_vars(Next)`; retain explicit process declarations. |

A procedure contributes only when syntactically reachable from that process.
Calls under a false condition still count. Direct calls retain source order;
transitive callees append in procedure declaration order. Unused procedures can
still have generated actions without receiving that caller's fairness clause.
Recursive calls remain runtime transitions, never source unrolling.

Rust callers use the same options and admission:

```rust
use dive_api::{TranslationFairness, TranslationRequest, translate_file};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut request = TranslationRequest::default();
    request.limits.fairness = TranslationFairness::Strong;
    let report = translate_file(Path::new("FairCall.tla"), &request)?;
    assert!(report.accepted());
    assert_eq!(report.to_json()["model_proved"], false);
    println!("{}", report.to_json());
    Ok(())
}
```

Admitted translation has `result="artifact"`, `accepted=true`, `exit_code=0`,
`model_proved=false`, `exhaustive=false`. The API validates the enclosing module.
It does not rewrite the source file. Failure returns a named code/span and no
admitted artifact. `DIVE-P0040` reports resource bounds, including formula
construction; `DIVE-P0090` reports unsupported scopes or layout.

The current boundary retains literal finite disjoint process domains, disjoint
names and proved per-ID parameter/local/stack/pc ownership. Scoped renaming,
macro/procedure interactions, sequential procedure modifiers and multiline,
commented or tabbed fairness domains remain refused. Parser/action/generated-byte
quotas remain global; fairness reachability work is also bounded by the generated
byte quota to prevent source-to-formula amplification.

Generating a fair Spec does not certify liveness execution. The curated finite
controls use `INIT Init` and `NEXT Next`, separately from the generated fairness
formula. Strict TLC-profile independent witness replay may still refuse runtime
record/string namespace provenance (#124). Canonical-profile replay and corrupted
trace controls are separately labelled evidence, never strict TLC agreement.
Primary artifacts rejected by SANY remain diagnostic raw evidence and are not
credited as admitted translation.

The complete reference study retains 480 capture rows (474 unique sources),
including original malformed modifiers and initial timeouts. Its 302 admitted
artifact rows, including six repeated timeout follow-ups, pass both SANY and
operator CST comparison. There are 118 finite model controls with matching
counts at 1/4/8 workers in both stores: 59 complete matches and 59 strict witnesses
pending #124. Separately labelled canonical replay validates 59 witnesses and
rejects 236 mutations; it provides no strict TLC-profile credit.

Developer evidence and the complete selected histories live in
`crates/dive-pluscal/procedure-fairness-acceptance/README.md`.
See [ADR 0046](decisions/0046-process-procedure-fairness.md),
[shared process procedures](pluscal-multiple-procedures.md),
[provenance](provenance.md) and [the parity ledger](tlc-parity.md).
