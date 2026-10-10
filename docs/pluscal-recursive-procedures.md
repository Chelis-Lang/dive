# Native recursive PlusCal procedures

`dive translate FILE.tla --output json` translates sequential self and mutual
procedure calls in Rust, including adjacent call/return and call/goto forms.
Procedure, parameter, local, global and label names must remain disjoint.
Translation does not execute the algorithm, unroll recursion, rewrite the file,
or prove termination. Both profiles use the same artifact-admission contract.

Save this as `RecursiveExample.tla`:

```tla
---- MODULE RecursiveExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Count(a = 0)
begin
  Entry: if a > 0 then call Count(a - 1); else x := x + 1; end if;
  Restore: x := x + 1; return;
end procedure;
begin
  Start: call Count(2);
  Finish: skip;
end algorithm; *)
====
```

The equivalent C-style source is:

```tla
---- MODULE RecursiveExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Count(a = 0) {
    Entry: if (a > 0) { call Count(a - 1); } else { x := x + 1; };
    Restore: x := x + 1; return;
  };
  { Start: call Count(2); Finish: skip; }
} *)
====
```

Run `dive translate RecursiveExample.tla --compat=tlc --output json`.
An admitted result has `result="artifact"`, `accepted=true`, `exit_code=0`,
`model_proved=false` and `exhaustive=false`. `module_source` contains the complete
frontend-validated enclosing TLA+ module; `artifact.actions` names the generated
actions. The example has actions `Entry`, `Restore`, `Start`, `Finish`, and
reaches `pc="Done"` with `x=4`. That behavior is a separate checking result,
not a claim made by translation.

Rust callers can use this program next to the saved module:

```rust
use dive_api::{TranslationRequest, translate_file};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = translate_file(Path::new("RecursiveExample.tla"), &TranslationRequest::default())?;
    assert!(report.accepted());
    assert_eq!(report.to_json()["model_proved"], false);
    println!("{}", report.to_json());
    Ok(())
}
```

`translate_source` provides the same contract for an in-memory enclosing module.
Parser, action and generated-output budgets are in `TranslationRequest.limits`.
They apply globally to main and all procedure bodies. A million recursive calls
in a possible execution do not create a million translation actions: only the
finite source is normalized. A translation budget failure is a source-spanned
`DIVE-P0040` resource rejection with no admitted artifact.

## Call and return semantics

Ordinary calls push a frame containing the callee's stored parameters/locals and
the caller continuation. Call arguments are assigned simultaneously, using the
pre-assignment values. Local initializers run in declaration order and observe
already assigned parameters and earlier locals. `return` uses its source
procedure owner to restore the frame and pop it. These operations also handle
self, two-procedure and larger cyclic call graphs; unused cycles are permitted.

An immediately adjacent `call Count(...); return` inside `Count` follows the
reference self-tail normalization. It keeps the current frame and continuation,
assigns the new parameters simultaneously, initializes locals and sets the
procedure entry. It does not grow the stack. Cross-procedure call/return instead
restores caller locals and replaces the stack head, preserving its continuation.
The pinned translator does not restore caller parameters in that cross form;
native translation preserves it. Admitted algorithm-level fairness options preserve the reference formulas;
this generated-model campaign checks `Init`/`Next` safety, not recursive
termination or liveness. Moving `return` behind a label changes these
operations. Reassigning a parameter/local and then tail-reassigning it in the
same action is rejected as a missing-label error where the reference rejects it.

A zero-parameter self-tail call is a measured upstream invalid-artifact case:
the pinned translator emits an empty conjunction and exits zero, but SANY
rejects that output. Both public profiles reject the module; it earns no
artifact or model-proof credit. The retained source and raw output are in
`recursive-procedure-acceptance/fixtures`; D-008 describes the limitation.
No silent `TRUE` repair or corrected-output default is introduced.

Automatic procedure labels, label modifiers, process-owned frames, ambiguous
names requiring renaming and unproved procedure/macro combinations remain named
refusals. Wrong arity/unknown callees are source rejections. Undefined expressions
in an unused procedure still make enclosing-module analysis reject the module.
Omitting `return` sets `pc="Error"` on fallthrough and can produce a deadlock;
translation never invents a return or proves that an infinite execution ends.

## Execution and witness qualifications

Finite execution controls compare clean pinned TLC against the admitted modules
at 1/4/8 workers, using fingerprint and exact stores. Counterexample validation
is a separate requirement. Strict `--compat=tlc` replay of stack/record-string
witnesses remains pending under #124 because the independent validator cannot
establish their runtime encounter namespace. The guard is unchanged.

Default/canonical replay is separately qualified evidence. A valid canonical
witness is not a fully validated TLC-profile witness. Corrupted traces must still
fail validation, and undecidable deadlock endings remain unsupported. Neither a
profile switch nor matching counts certifies a pending ending.

Truly unbounded recursive stacks/counters do not have a complete state graph.
The developer campaign retains clean primary timeouts and bounded native graph
capture stops, with `model_proved=false` and `exhaustive=false`. The bounded
capture API reports a resource limit when it reaches its state/edge/generated
budget, or a named runtime-namespace refusal for the staged TLC capture path.
It does not export a partial graph as complete. A TLC-mode increasing integer
can eventually overflow, so a timed-out prefix is not proof of infinite TLC
execution. Ordinary CLI safety search has
no user state-count limit in this increment; use a bounded backend/API for
potentially infinite models rather than expecting a proof from a timeout.

## Evidence and remaining scope

The capture-first selection keeps all 116 C/P inputs: 101 primary translations,
15 primary rejections and all 10 SANY-invalid primary artifacts. Native token
accounting is 86 artifact matches, 15 rejection matches, nine named refusals and
six diagnostic invalid-reference dispositions. Invalid primary output is never
credited. Enclosing SANY acceptance and operator CST grouping are checked
separately from significant-token equality.

See `crates/dive-pluscal/recursive-procedure-acceptance`,
[ADR 0043](decisions/0043-pluscal-recursive-procedure-calls.md), and the immutable
[earlier nonrecursive study](pluscal-procedure-graphs.md). Process-owned/scoped
renaming, historical options, file rewriting/checksum/format/map parity and full
strict witness provenance remain pending; F-034/F-035 remain partial.

Measured execution accounts for 146 finite-model rows and 876 width/store
controls: 72 complete models, 72 strict invariant witnesses pending and two strict
deadlock witnesses pending. Separately qualified canonical replay validates 74
witnesses and rejects 296 mutants. Both selected canonical fallthrough deadlocks
are decidable; the initial inherited contrary expectation is retained. The 192
bounded controls comprise 96 default resource stops and 96 strict runtime-name
capture refusals. All ten primary-invalid artifacts are rejected by CLI/API in
both profiles. These figures describe this selection, not full translator parity.

Worker widths in this study are requested widths. TLC encounter-order paths may
run inline with the actual execution mode and reason recorded in each checker
document. These controls prove result agreement, not physical parallel scaling.
No performance claim follows from this artifact/execution campaign.
