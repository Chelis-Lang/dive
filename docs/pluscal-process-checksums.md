# Checksums for bounded process translations

Full in-memory rewriting derives the pinned algorithm CRC from native
post-translation AST serialization for bounded finite process blocks. One or
multiple disjoint literal-ID/membership blocks use their source order, local
declarations, labels and declared weak/strong fairness. This extends the
sequential checksum path; it does not change the CRC algorithm or format policy.

Save this P-style example as `Process.tla`:

```tla
---- MODULE Process ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable g = 0;
process Worker \in {1, 2}
variable z = 0;
begin
  Start: z := z + 1;
  Finish: g := z;
end process;
end algorithm; *)
====
```

The C-style local declaration belongs before the process body brace:

```tla
---- MODULE Process ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable g = 0;
  process (Worker \in {1, 2})
  variable z = 0;
  {
    Start: z := z + 1;
    Finish: g := z;
  }
} *)
====
```

Run either source without changing the file:

```sh
dive translate Process.tla --rewrite --checksums full --compat=tlc --output json
cargo run -p dive-api --example pluscal_process_checksum -- Process.tla --compat=tlc
```

An admitted report returns `result: "replacement"` and the generated module in
`replacement.module_source`, with `algorithm_checksum_verified` and
`body_checksum_verified` true. Native source admission and reference CRC
derivation are separate checks. `model_proved` and `exhaustive` remain false.
The process locals and program counters are indexed by their actual process
IDs in generated TLA; checksum AST bytes retain the original PlusCal expressions.
Shared variables remain shared. Formatter-byte and precise mapping parity are
not claimed, and this API does not write to disk.

The same contract is available to Rust callers:

```rust
use dive_api::{RewriteRequest, rewrite_source};
let source = std::fs::read_to_string("Process.tla")?;
let report = rewrite_source("Process.tla", &source, &RewriteRequest::default());
assert!(report.accepted());
assert_eq!(report.to_json()["model_proved"], false);
println!("{}", report.to_json());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Declared process fairness and `L:+`/`L:-` label modifiers are serialized in lexical
order within their owner. Unproved CLI fairness overrides and automatic-label
normalization remain named checksum refusals. General renaming, definitions,
macros, procedures and branch-internal labels also refuse; no placeholder Full
checksum is invented. Multiline process expressions may have exact raw AST
serialization while generation still refuses their unproved layout. Raw checksum
evidence is not public artifact admission. Invalid primary artifacts remain
uncredited.

`--checksums tla` requests the separate body-only policy. It never silently
replaces a refused Full request; in TLC mode it is labelled `tlc+overrides`.
The pinned `--fair algorithm` CRC still hashes only `fair` (D-020). Its equality
does not establish source identity, integrity or a certificate. This limitation
is preserved in both profiles. Default rewriting preserves source newlines;
TLC mode normalizes LF. Hard document, output, action and parser budgets remain
global, with named `Resource` failures instead of partially admitted output.

Evidence and all original refusals are retained under
`crates/dive-pluscal/process-checksum-acceptance/`. Exact AST bytes, generated
operator CST, SANY and Validator controls must be reported separately from finite
model/witness checks. F-034/F-035 remain partial and audit-required. See the
[rewrite contract](pluscal-rewriting.md), [process translation guide](pluscal-processes.md)
and [ADR 0047](decisions/0047-bounded-translation-block-editing.md).

The retained selection is 224 sources, including 28 separately captured owner,
name-width and default-initializer controls. Raw serialization has 160 exact matches,
36 reference parser rejections and 28 named refusals. Public Full admission
accepts 152 unique sources per profile, each with strict whole-module CST, SANY
and Validator evidence. Eight additional raw matches remain generation refusals.
Forty-four finite safety models give 528 profile/worker/store controls, 22 strict
independently valid witnesses and 88 rejected witness mutants. These are separate evidence;
process fairness in emitted formulas is not certified by safety-only checking.
All source and corrupted checksum controls retain their measured disposition.

Opposite +/- modifiers on disjoint process owners retain separate source-owned
lists. A retained generator failure with `defaultInitValue` exposed the pinned
singleton-UNCHANGED spelling rule: names longer than five UTF-16 units use
`UNCHANGED name`. The source-backed repair preserves operator CSTs at the
five/six boundary and in branches. Default-initializer name conflicts retain
their measured reference rejection and native named refusal; no scoped renaming
is guessed. All earlier failed artifacts and gate producers remain archived.

The counts above belong to the immutable process-checksum producer identified
in the study delivery receipt. They are retained producer evidence; combining
this code with later temporal, capture or recovery changes requires fresh
source-bound controls and the current required conformance selection.
