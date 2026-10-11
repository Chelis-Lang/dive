# Full checksums for nested and multiline PlusCal

`dive translate --rewrite` now derives both reference CRCs for supported
sequential `elsif` and multiple-`with` translations. The bounded serializer
also reproduces multiline expression ASTs. Bounded non-junction sequential
multiline generation is now described in [the multiline guide](pluscal-multiline.md);
unproved junction/substitution layouts remain named refusals. CRC derivation, generated-module
admission and model checking are separate results.

Save the P-style example as `ChecksumNested.tla`:

```tla
---- MODULE ChecksumNested ----
EXTENDS Integers
(* --algorithm A
variables x = 0;
begin
  L: with a = 1, b = 2 do
       if x = 0 then x := a + b;
       elsif x = 1 then x := 2;
       else skip;
       end if;
     end with;
  E: skip;
end algorithm; *)
====
```

The equivalent C-style module is:

```tla
---- MODULE ChecksumNested ----
EXTENDS Integers
(* --algorithm A {
  variables x = 0;
  {
    L: with (a = 1, b = 2) {
         if (x = 0) { x := a + b; }
         else { if (x = 1) { x := 2; } else { skip; } }
       };
    E: skip;
  }
} *)
====
```

The C compound statement has a semicolon before `E`. A trailing separator in
`with(a = 1,)` or `with(a = 1;)` is also accepted, matching the reference.
An empty binder, absent first binder or doubled separator is rejected.
P syntax permits an empty `else` after a nonempty `then`; the emitted `TRUE`
conjunct matches the reference. An empty `then` with a nonempty `else` parses
but generation is explicitly unsupported. C empty bodies and P conditionals
with both branches empty are rejected.

Run either source without overwriting it:

```sh
dive translate ChecksumNested.tla --rewrite --compat=tlc --output json > plan.json
```

An accepted report has `result: "replacement"`, `algorithm_checksum_verified`
and `body_checksum_verified` set to true, and the resulting module under
`replacement.module_source`. Both `model_proved` and `exhaustive` remain false.
The report retains the exact serialized AST used for CRC derivation. The input
file is unchanged; `--write` remains unsupported. The default profile preserves
the source's first newline style; `--compat=tlc` normalizes LF.

The following multiline junction example remains a generation refusal. Save it as
`ChecksumLines.tla`:

```tla
---- MODULE ChecksumLines ----
EXTENDS Integers
(* --algorithm A
variable x = 0;
begin
  L: await /\ TRUE
           /\ x = 0;
  E: skip;
end algorithm; *)
====
```

```sh
dive translate ChecksumLines.tla --rewrite --output json > lines-plan.json
```

This command exits 3 with a named unsupported multiline-layout/substitution
diagnostic. It does not produce an admitted replacement. Rust callers can
inspect the separately proved serializer through
`dive_pluscal::algorithm_checksum_at`; that raw derivation grants no artifact
admission. The serializer retains blank/comment lines between significant tokens. Tabs
advance to eight-column stops and ASCII `/\` or `\/` tokens record the
reference's normalized UTF-16 columns. LF and CRLF are supported; bare CR is
refused until its different line contract is proved. Those details affect the
CRC even when they leave TLA+ meaning unchanged. Artifact admission and operator
grouping are checked separately; an exact serialized AST is not permission to
credit a SANY-invalid generated module.

Rust callers use the same bounded, in-memory contract:

```rust
use dive_api::{RewriteRequest, rewrite_source};
let source = "---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:with a=1,b=2 do x:=a+b; end with; E:skip; end algorithm; *)\n====\n";
let mut request = RewriteRequest::default();
request.translation.compat_tlc = true;
let report = rewrite_source("R.tla", source, &request);
assert!(report.accepted());
assert_eq!(report.to_json()["algorithm_checksum_verified"], true);
assert_eq!(report.to_json()["model_proved"], false);
```

For raw multiline CRC evidence without artifact admission:

```rust
use dive_pluscal::{Code, TranslationOptions, algorithm_checksum_at, translate};
let source = "--algorithm A variable x=0; begin L:x:=1 +\n2; E:skip; end algorithm;";
let options = TranslationOptions::default();
let checksum = algorithm_checksum_at(source, 0..source.len(), &options)?;
assert!(checksum.serialized_ast.contains("\"+\"\n"));
assert!(translate(source, &options)
    .is_err_and(|error| error.diagnostic.code == Code::Unsupported));
```

Existing source/token/node limits remain in force. Serialized output honors
lower caller output budgets and has a 4 MiB hard ceiling. Unknown locations,
proof-step-shaped tokens, unproved scoped name fixes, macros, processes,
procedures, definitions and branch-internal labels receive named refusals.
Use explicit `--checksums tla` only when broader generation is supported: it
disables the algorithm CRC instead of inventing one.

For a `--fair algorithm`, the pinned implementation hashes `"fair"` alone.
`fair_header_crc_only` exposes that weakness. A matching CRC does not establish
source identity, integrity, semantic equivalence or model correctness. The
matching behavior is retained; no stronger checksum default has been approved.
See [D-020](tlc-differences.md#d-020-fair-algorithm-checksums-do-not-detect-source-edits),
[block editing](pluscal-rewriting.md), [ADR 0047](decisions/0047-bounded-translation-block-editing.md)
and [the retained study](../crates/dive-pluscal/checksum-expression-acceptance/README.md).
Full PlusCal shapes, scoped renaming, exact generator formatting/file writes/maps
and complete family certification remain pending.

The retained main study has 320 inputs: 224 exact raw AST/CRC matches, 14 named
serialization refusals and 82 primary rejections. Public Full rewriting admits
86 replacements per profile, names 140 unsupported requests and rejects 94
inputs, including 12 invalid-primary outputs. All 16 SANY-invalid primary
artifacts remain uncredited. Two additional old-test boundary controls prove
exact elsif AST/CRC and generated operator structure while retaining an
unproved macro refusal. Twelve finite generated models match verdict/counts
across 72 worker/store controls; six strict witnesses validate independently
and 24 corrupted traces reject. These are bounded evidence, not family parity.
