# Shared procedures called by multiple finite processes

`dive translate FILE.tla --output json` admits multiple finite process blocks
calling shared procedures in C or P syntax. Each process ID owns its procedure
parameters, locals, stack and program counter. Procedure actions and storage
declarations are generated once, regardless of the number of process blocks.
This slice requires disjoint names and IDs; it does not rename scopes or prove
termination.

Save this P-style example as `MultipleCallExample.tla`:

```tla
---- MODULE MultipleCallExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Add(a = 0)
begin
  Entry: x := x + a;
  Return: return;
end procedure;
process One = 1
begin
  First: call Add(1);
  FinishOne: skip;
end process;
process Two \in {2}
begin
  Second: call Add(self);
  FinishTwo: skip;
end process;
end algorithm; *)
====
```

The equivalent C-style module is:

```tla
---- MODULE MultipleCallExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Add(a = 0) {
    Entry: x := x + a; Return: return;
  };
  process (One = 1) { First: call Add(1); FinishOne: skip; }
  process (Two \in {2}) { Second: call Add(self); FinishTwo: skip; }
} *)
====
```

Run `dive translate MultipleCallExample.tla --compat=tlc --output json`.
An admitted report has `result="artifact"`, `accepted=true`, `exit_code=0`,
`model_proved=false` and `exhaustive=false`. The frontend validates `module_source`
before returning it; translation does not write a replacement file or search a
state graph. To inspect an independent finite execution, save `module_source`
and check it with `INIT Init` and `NEXT Next` in its configuration. Every complete
execution of this example reaches both IDs' `pc="Done"` with `x=3`.

Rust callers use the same admission contract:

```rust
use dive_api::{TranslationRequest, translate_file};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = translate_file(Path::new("MultipleCallExample.tla"), &TranslationRequest::default())?;
    assert!(report.accepted());
    assert_eq!(report.to_json()["model_proved"], false);
    println!("{}", report.to_json());
    Ok(())
}
```

`translate_source` accepts an in-memory enclosing module. Both profiles require
the same artifact admission; compatibility mode cannot admit invalid upstream
output. `TranslationRequest.limits` accounts parser, action and output quotas
globally across all bodies and procedures. Resource failure uses `DIVE-P0040`;
unproved feature paths use `DIVE-P0090`, with original source spans and no admitted
module. Runtime recursion remains in generated transitions, never source
unrolling.

## Ownership, initialization and order

`ProcSet` is the union of the source process IDs/domains. Procedure slots and
`stack` initialize once over this union; process locals retain constant-scalar
or member-indexed ownership. Empty/reversed member domains remain empty. Decimal
integer intervals and unescaped literal string domains are supported when
disjointness is provable without enumerating them.

Ordinary calls save only the caller ID's frame; return restores that ID's stored
slots and continuation. Nested and recursive calls share procedure definitions
without sharing frames. Same-procedure adjacent call/return preserves the frame;
cross-procedure adjacent call/return uses the pinned head replacement. Arguments
are simultaneous pre-assignment values; procedure local initialization observes
assigned parameters and earlier locals. Explicit source `stack`/`pc` expressions
stay as written, rather than gaining an extra implicit index.

Procedure actions precede process actions in source-normalized artifact order.
`Next` places constant-ID process wrappers first, then one quantified procedure
disjunction over `ProcSet`, then member-process wrappers. Source interleaving does
not override this pinned scheduling order. Global `--fair algorithm` generates
`WF_vars(Next)`; artifact evidence does not certify liveness execution.

## Limits and evidence

Overlapping or unproved domains, duplicate/disambiguated scopes, cross-process
local reads and procedure references to process-local storage remain named
refusals. Unproved macro bindings and historical options such as `-termination` are not admitted here.
The unimplemented command-line `-termination` option exits 3 with a named
stderr refusal before loading the module, even with `--output json`; no artifact
report is produced. Wrong arity, unknown callee and invalid label transfers are
source rejections.
Falling off a procedure retains `pc[id]="Error"`; no return is invented.

All selected raw translator/error/SANY artifacts remain visible. A pinned
translation may exit zero yet produce invalid TLA—for example generated
`ProcSet` conflicts or zero-parameter self-tail output. Both public profiles
reject these; diagnostic-only raw matches never earn admitted artifact credit.
Token comparisons are backed by enclosing SANY and operator CST checks, retaining
junction grouping and prime scope. Pretty width, file rewriting, checksums and
the original map format remain separate pending contracts.

Finite `Init`/`Next` campaigns compare clean TLC with requested native 1/4/8
workers and fingerprint/exact stores. Actual execution may be inline for runtime
names and is reported; no scaling claim follows. Strict independent replay
under `--compat=tlc` remains pending where the #124 guard cannot establish stack
record/string encounter history. The guard is unchanged. Default/canonical
positive and corrupted witnesses are separately qualified, never strict-profile
certification. Timed-out recursive primary prefixes and native resource stops
are nonproof evidence, not infinite-execution or complete-search claims.

See [ADR 0045](decisions/0045-pluscal-shared-process-procedures.md),
[the retained study](../crates/dive-pluscal/multiple-procedure-acceptance/README.md),
[one-process procedures](pluscal-process-procedures.md) and
[the parity ledger](tlc-parity.md). F-034/F-035 remain partial and audit-required.

The historical 166-source C/P campaign admits 94 artifacts, matches 15 source
rejections, refuses 49 unproved inputs by name, and records eight diagnostic
invalid-reference comparisons. All eleven SANY-invalid primary outputs remain
uncredited. Each admitted artifact is separately checked against SANY and the
primary operator CST. The 142 finite model rows match verdicts and counts at
requested 1/4/8 workers in both stores: 74 complete searches and 68 witnesses
whose strict replay remains pending (66 invariant, two deadlock). The 68 valid
canonical-profile replays and 272 rejected mutants are separate qualified
evidence, never strict TLC witness credit or liveness certification.

Process-declared/option fairness and process-owned procedure modifiers are
extended by the [fair procedure increment](pluscal-procedure-fairness.md).
The earlier unfair-only capture and its dispositions remain historical evidence;
fairness generation alone does not certify liveness execution.

## Current integrated fairness coverage

The current translator also admits the proved process/procedure fairness cases
described in [procedure fairness](pluscal-procedure-fairness.md). Fresh comparison
of this guide's original selection now matches 106 artifacts; the older counts
above retain their original source qualification. All 14 newly admitted cases
across the one-process and shared-procedure selections pass separate pinned
SANY and operator-CST checks.

The [combined-source receipt](../conformance/probes/native-parity-integration/current-controls/pluscal-process-fairness-integration/verification.json)
records 906 passing tests, 302 fair translation artifacts and distinct producer
identities for execution and test-expectation reconciliation. It retains the
initial failed gate. Strict runtime-namespace witnesses remain pending under
#124; these results certify neither liveness execution nor full PlusCal parity.
