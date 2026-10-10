# Native PlusCal procedure graphs

This guide preserves the earlier bounded delivery and its original evidence.
The current disjoint sequential path also admits recursion; see
[recursive procedure use and limitations](pluscal-recursive-procedures.md).


`dive translate FILE.tla --output json` admits multiple sequential procedures
with disjoint names and a nonrecursive call graph. Both PlusCal syntaxes use
native Rust translation. Save this as `ProcedureGraphExample.tla`:

```tla
---- MODULE ProcedureGraphExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Outer(a = 0)
variable u = 1;
begin
  OuterEntry: call Inner(a + u);
  OuterReturn: return;
end procedure;
procedure Inner(b = 0)
variable v = 2;
begin
  InnerEntry: x := b + v;
  InnerReturn: return;
end procedure;
begin
  Start: call Outer(3);
  Finish: skip;
end algorithm; *)
====
```

The equivalent C-style module is:

```tla
---- MODULE ProcedureGraphExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Outer(a = 0) variable u = 1;
  { OuterEntry: call Inner(a + u); OuterReturn: return; };
  procedure Inner(b = 0) variable v = 2;
  { InnerEntry: x := b + v; InnerReturn: return; };
  { Start: call Outer(3); Finish: skip; }
} *)
====
```

Run `dive translate ProcedureGraphExample.tla --compat=tlc --output json`.
The result is `artifact`, `accepted=true`, `exit_code=0` at `artifact-analysis`,
with `model_proved=false` and `exhaustive=false`. The validated enclosing module
is `module_source`. Translation returns data; it does not rewrite the file or
run Java. Both profiles use the same admitted translation contract.

Rust callers can run this next to the saved module:

```rust
use dive_api::{TranslationRequest, translate_file};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = translate_file(Path::new("ProcedureGraphExample.tla"), &TranslationRequest::default())?;
    assert!(report.accepted());
    assert_eq!(report.to_json()["model_proved"], false);
    println!("{}", report.to_json());
    Ok(())
}
```

`translate_source` also accepts an in-memory enclosing module. Configure parser,
action and generated-output budgets in `TranslationRequest.limits`.
Normalization owns one private source arena and charges normalized-node and
action budgets globally across main and every procedure. Resource exhaustion
returns a source-spanned `DIVE-P0040` error, without an artifact. Unknown call
targets or wrong argument counts are rejections. Unproved namespaces return
`unsupported` (exit 3); the historical cycle refusal is superseded by the
current recursive guide linked above.

## Frames, names and control flow

Each source procedure owns its parameters, locals, entry action and return
operations. Actions appear in procedure declaration order, then main-body order.
`Init` initializes the stored parameter/local variables. An ordinary nested call
pushes the callee's old values and the caller continuation on `stack`, assigns
parameters simultaneously, initializes locals, then sets the callee entry `pc`.
Returning restores the frame's continuation, locals and parameters, and pops it.
The example eventually assigns `x=6` and reaches `Done`.

An adjacent `call Inner(...); goto Label` pushes the explicit continuation.
An adjacent `call Inner(...); return` inside a procedure has a separate reference
normalization: it replaces the stack head, preserves its continuation, restores
caller locals and assigns callee parameters. It does not first push and then
pop an ordinary call. The pinned translator does not restore caller parameters
in this form; native translation preserves that normalization. Saved frame
values and procedure variables remain observable, so simplifying them away
would change the model. Simultaneous call arguments use the pre-assignment
values; later local initializers observe already initialized parameters/locals.

Names must be disjoint across globals, every parameter/local, procedure names
and labels; generated names such as `pc` and `stack` are reserved. This slice
does not implement the reference renaming machinery. All procedure labels are
explicit. This historical slice refused direct, mutual and tail recursion,
including unused cycles; the current recursive guide supersedes that restriction.
Process-owned stacks,
automatic procedure labels, procedure-label fairness modifiers and unproved
procedure/macro combinations remain unsupported. Existing admitted branch,
loop, `with`, parallel assignment and algorithm-level fairness forms remain
available.

Omitting a procedure's final `return` makes fallthrough set `pc="Error"`, as the
pinned translator does. It does not invent a return. Checking that artifact can
produce a deadlock (TLC exit 11); an admitted translation is never a model proof.
Invalid generated modules are rejected by enclosing-module analysis, including
unused procedures with undefined expressions. Invalid primary output is retained
only as diagnostic evidence, never credited as an admitted artifact.

## Evidence and remaining limits

The retained capture-first study is
`crates/dive-pluscal/procedure-graph-acceptance`. It keeps all 120 C/P sources,
primary translator/normalization errors, generated modules and SANY dispositions.
The token campaign accounts for 68 admitted artifacts, 19 source rejections,
29 named refusals and four diagnostic invalid-reference dispositions. Eight
primary outputs fail SANY and remain uncredited. Every admitted artifact is
also checked as an enclosing module with SANY and an operator CST comparison,
which retains junction indentation and grouping rather than tokens alone.

The curated campaign retains 130 generated-model rows: 64 complete searches,
62 invariant witnesses and four implicit-fallthrough deadlocks. Clean TLC
verdict/count/depth and native results match at 1/4/8 workers with fingerprint
and exact stores (780 controls). The original strict report has 64 full complete
matches and 66 replay disagreements; these remain visible.
The initial campaign incorrectly expected implicit callee fallthrough to succeed;
the clean pinned run returned deadlock. That failed campaign and raw output are
kept beside the corrected deadlock controls, not discarded or relabelled proof.

Strict TLC-profile replay of procedure stack traces remains pending under #124:
the independent validator cannot prove the runtime string encounter namespace
from the supplied trace. This guard is unchanged, and a pending witness earns no
fully validated TLC-violation credit. Separately labelled default/canonical
replay provides qualified invariant-witness and corrupted-trace checks; it does
not certify the TLC profile. Deadlock replay is also pending where the independent
validator cannot decide the ending. Neither profile may claim those undecided
endings as independently valid.

Full procedure scopes/renaming, recursion, process storage, historical options,
file rewriting/checksum, exact formatting/mapping contracts and strict namespace
provenance remain pending. F-034/F-035 remain partial. See
[ADR 0042](decisions/0042-pluscal-nonrecursive-procedure-graphs.md) and the
[earlier single-procedure evidence](pluscal-procedures.md).

The separately qualified canonical replay checks 62 concrete invariant witnesses
as valid and rejects 264 corrupted traces (four for every invariant/deadlock
witness). Four original deadlock endings remain unsupported even in this
profile. Every switched-profile control records `strict_tlc_credit=false`.
