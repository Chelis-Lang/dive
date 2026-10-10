# Procedures called by a finite PlusCal process

`dive translate FILE.tla --output json` now admits procedure calls from one
finite process block with an unfair process declaration in C or P syntax.
Procedure parameters, locals,
stack and program counter belong to a process ID. Ordinary and recursive calls,
return, and adjacent self-tail call/return use that owner's slots. This bounded
path requires disjoint names; it does not rename scopes or prove termination.

Save this P-style example as `ProcessCallExample.tla`:

```tla
---- MODULE ProcessCallExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Count(a = 0)
begin
  Entry: if a > 0 then call Count(a - 1); else x := x + 1; end if;
  Restore: x := x + 1; return;
end procedure;
process Worker = 1
begin
  Start: call Count(2);
  Finish: skip;
end process;
end algorithm; *)
====
```

The equivalent C-style example is:

```tla
---- MODULE ProcessCallExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Count(a = 0) {
    Entry: if (a > 0) { call Count(a - 1); } else { x := x + 1; };
    Restore: x := x + 1; return;
  };
  process (Worker = 1) {
    Start: call Count(2); Finish: skip;
  }
} *)
====
```

Run `dive translate ProcessCallExample.tla --compat=tlc --output json`.
An admitted report has `result="artifact"`, `accepted=true`, `exit_code=0`,
`model_proved=false` and `exhaustive=false`. `module_source` is the enclosing
frontend-validated module, not a replacement file written to disk. The actions
are `Entry`, `Restore`, `Start`, `Finish`; `Count(self)` collects the procedure
steps and `Next` quantifies procedure execution over `ProcSet`. The example's
separate finite execution reaches `pc[1]="Done"` with `x=4`. Translation itself
claims no state search or termination proof.

Rust callers use the same admission path:

```rust
use dive_api::{TranslationRequest, translate_file};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = translate_file(Path::new("ProcessCallExample.tla"), &TranslationRequest::default())?;
    assert!(report.accepted());
    assert_eq!(report.to_json()["model_proved"], false);
    println!("{}", report.to_json());
    Ok(())
}
```

`translate_source` accepts an in-memory enclosing module. Both profiles apply
identical artifact validation; `--compat=tlc` is not permission to admit invalid
upstream output. Requests expose parser, action and output budgets through
`TranslationRequest.limits`, accounted globally across the process and procedures.
Failure reports carry a stable code and original source span, without an admitted
module: resource limits use `DIVE-P0040`; unproved features use `DIVE-P0090`.

## Storage and transfers

A process with `Worker = 1` calls through index `1`. A process set, such as
`Worker \in {1,2}`, calls through bound `self`; literal string IDs and decimal
intervals are also admitted. Empty/reversed domains are retained as empty
process sets, not silently expanded. Procedures execute as `Label(self)` even
for constant-ID callers. Their slots initialize over `ProcSet`; process locals
remain scalar for constant IDs and indexed for member IDs.

An ordinary call saves the callee's stored parameters/locals and caller
continuation in `stack[id]`. Arguments use simultaneous pre-assignment values;
local initialization then observes assigned parameters and earlier locals.
Return restores the owning slots and continuation, and pops only that ID's
stack. A same-procedure adjacent `call Count(...); return` preserves the frame,
rebinds parameters and initializes locals; cross-procedure adjacent call/return
uses the pinned head-replacement rules. Recursion stays in runtime transitions,
never translation-time expansion.

Explicit source reads of `stack` are left as written by the reference:
`Len(stack[self])` means that owner's stack, while `Len(stack)` does not gain an
implicit index. Internal generated transfers still index the owned stack.

## Supported limits and refusals

This increment supports one finite process block and disjoint procedure names,
parameter/local/global names and labels. Multiple process blocks with procedures,
procedure references to process-local storage, process-declared or option-selected fairness,
automatic procedure labels, label modifiers, scoped renaming and unproved
macro/binder interactions remain named refusals. Wrong call arity, unknown
callee and missing-label transfers are source rejections. Falling off a
procedure produces `pc[id]="Error"` as in the reference; no return is invented.

The pinned translator can exit zero with a SANY-invalid module—for example,
zero-argument self-tail calls emit an empty conjunction. Both public profiles
reject these artifacts. All invalid primary outputs remain diagnostic-only
in the complete source selection; none counts as an artifact or model proof.
A corrected-output option has not been adopted.

## Checking evidence is separate from translation

The study keeps complete source/normalization/token, enclosing-module SANY and
operator-CST evidence. CST grouping checks retain junction indentation and prime
scope, rather than treating significant-token equality as semantic proof.
Finite generated models compare clean single-worker TLC with requested native
1/4/8 workers in fingerprint/exact stores. Actual execution mode and inline
encounter-order reasons remain in each checker report; no scaling claim follows.

Strict `--compat=tlc` independent replay of stack/record-string traces remains
pending under #124 where runtime string encounter history cannot be established.
The existing guard is unchanged. Default/canonical positive and corrupted
witness controls are separately labelled, never TLC-profile certification.
Undecidable endings remain unsupported. Bounded graph capture of an unbounded
recursive execution reports resource/namespace limits with `exhaustive=false`
and `model_proved=false`; a timed-out TLC prefix proves no infinite execution.

See [ADR 0044](decisions/0044-pluscal-process-procedure-ownership.md),
[the retained study](../crates/dive-pluscal/process-procedure-acceptance/README.md),
[sequential recursion](pluscal-recursive-procedures.md) and
[the parity ledger](tlc-parity.md). Full PlusCal translation, exact formatting,
file rewriting/checksums/maps and full strict witness certification remain
pending; F-034/F-035 stay partial and audit-required.

Current evidence retains 130 source rows in three immutable selections: 80
admitted token/SANY/operator-CST artifacts, ten matching source rejections, 30
named refusals and ten invalid-reference diagnostic dispositions. All fourteen
SANY-invalid reference outputs remain uncredited. The 130 finite model rows have
66 complete matches and 64 strict pending witnesses; separately qualified
canonical replay validates 64 witnesses and rejects 256 mutants. This is bounded
coverage, not full translator or strict witness certification.

A separate final two-row C/P study also admits algorithm-level `--fair algorithm`.
It generates the pinned global `WF_vars(Next)` formula, independently of the
process declaration. Both generated operators match SANY and CST structure;
four finite Init/Next controls match (two complete, two strict pending witnesses).
Two canonical witnesses and eight invalid mutants are separate qualified
evidence. This establishes formula generation, not liveness execution or
process/procedure fairness. The aggregate delivery contains 132 source rows,
82 artifacts and 134 model rows; the original 130-row study remains intact.
