# Process options and Full checksums

`dive translate --rewrite` can derive the pinned PlusCal AST checksum for
bounded finite process algorithms using `-nof`, `-wf`, `-sf`, `-wfNext`,
`-label` and `-labelRoot`. The default checksum policy is `full`. The returned
replacement stays in memory; it does not overwrite your file or prove your model.
The same supported generation runs in both profiles.

Save this as `ProcessOptions.tla`:

```tla
---- MODULE ProcessOptions ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A
variables g = 0;
process P \in {1, 2}
begin
  g := 1;
  g := 2;
end process;
end algorithm; *)
====
```

The C-syntax counterpart is:

```tla
---- MODULE ProcessOptions ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {
variables g = 0;
process (P \in {1, 2}) {
  g := 1;
  g := 2;
}
} *)
====
```

```sh
cargo run -p dive-cli --release -- translate ProcessOptions.tla \
  --rewrite --checksums full -label -labelRoot Auto -wf --output json
```

An admitted report has `result: "replacement"`, `accepted: true`,
`algorithm_checksum_verified: true` and `body_checksum_verified: true`.
`model_proved` and `exhaustive` remain false. `replacement.module_source`
contains generated labels `Auto1` and `Auto2`, process actions and weak process
fairness. `translation_options` records the exact settings used to derive the
CRC. `--compat=tlc` preserves the same proved serialization and uses the pinned
newline behavior. Without `-label`, this missing-label source is rejected;
`-labelRoot` alone never turns insertion on.

For Rust callers, set the typed options and inspect the report:

```rust
use dive_api::{RewriteRequest, TranslationFairness, rewrite_source};
let source = std::fs::read_to_string("ProcessOptions.tla")?;
let mut request = RewriteRequest::default();
request.translation.limits.insert_missing_labels = true;
request.translation.limits.label_root = "Auto".into();
request.translation.limits.fairness = TranslationFairness::Weak;
let report = rewrite_source("ProcessOptions.tla", &source, &request);
assert!(report.accepted());
assert_eq!(report.to_json()["model_proved"], false);
# Ok::<(), Box<dyn std::error::Error>>(())
```

A Cargo-owned executable of this API example also checks the same source:

```sh
cargo run -p dive-api --release --example pluscal_process_options -- \
  ProcessOptions.tla -label -labelRoot Auto -wf
```

Select only one fairness flag. The CLI rejects even repeated fairness flags
before opening the file; the Rust API represents one choice as an enum.
A declared `fair`/`fair+` process retains its declaration in the reference AST:
`-wf`/`-sf` supply fairness only to otherwise unfair processes. `-nof` suppresses
emitted process fairness but does **not** erase a declared fairness field from
the checksum AST. `-wfNext` changes the generated next-action fairness without
changing the process AST fairness. These are distinct parser and generator
contracts. Separate owners keep separate +/- label lists, and inserted labels
skip existing names using the selected prefix.

Full serialization still refuses scoped renaming, conflicting labels/default
initializer names, definitions, unproved macro layouts, procedures and unproved branch-label
or expression-layout shapes. It does not certify exact pretty-printing,
translation maps or filesystem writes. Global parser/node/action/output limits
still apply before a partial artifact or CRC can escape. Explicit `--checksums
tla` remains the separate body-only option, reported as an override in TLC mode.

## Validator context matters

Pinned `pcal.trans -wf File.tla` does not save its command-line options in the
translation header. A fresh unconfigured `pcal.Validator` can therefore report
`PCAL_DIVERGENCE_EXISTS` for its own unmodified generated file. Configuring the
same options gives `NO_DIVERGENCE`. Automatic-label and custom-prefix options
have the same issue. Store the generation options beside the artifact and
configure the reference validator consistently; a header alone does not encode
them. See [D-021](tlc-differences.md#d-021-pluscal-checksums-need-the-generation-options).

The reference also reads `PlusCal options (...)` declarations in source.
Native translation/checksum generation currently refuses that source context
by name until option-comment parsing and precedence are proved. Pass supported
settings explicitly through the CLI/API instead; the refusal is never a claim
that the source is invalid. The original ignored-option native failures remain
archived. The `--fair algorithm` CRC hashes only `fair` under the pinned build
([D-020](tlc-differences.md#d-020-fair-algorithm-checksums-do-not-detect-source-edits));
matching that CRC does not establish source identity, even when validation says
`NO_DIVERGENCE`.

## Evidence and remaining scope

Clean pinned captures live in
`crates/dive-pluscal/process-option-checksum-acceptance/`. They retain every
selected source/option pair, rejected option combination, parse/name/translated
AST, original native failure and configured/unconfigured Validator result.
Raw AST/CRC equality, public module admission, whole-module CST/SANY validation
and finite model/witness checks are separate stages. Finite INIT/NEXT checking
does not certify the fairness formulas' liveness semantics. This increment
leaves the PlusCal parity family partial; broader option comments, name fixes,
procedures and formatting/checksum interoperability still need their own proof.
See [process checksum scope](pluscal-process-checksums.md),
[rewriting](pluscal-rewriting.md) and
[ADR 0047](decisions/0047-bounded-translation-block-editing.md).

The frozen selection contains 524 source/option pairs: 320 exact raw AST/CRC
matches, 48 named refusals, 128 source/label rejections and 28 incompatible CLI
option selections. Each profile admits 320 Full replacements; all 640 have
strict whole-module CST, SANY and consistently configured Validator controls.
Separate finite safety checks cover 24 models at 1/4/8 workers in fingerprint
and exact stores (288 runs), with 12 strict independently valid witnesses and
48 invalid mutants. These do not establish liveness or source identity.
Forty-four forged reports and 12 header/body/source corruptions are rejected or
retain the explicitly recorded D-020 weakness. Sixteen runnable CLI/Rust guide
controls and 32 named source-context surface refusals have separate receipts.

Recognized source-option declarations are inspected in the original bounded
file, including an existing translation block, before native rewriting masks
old output. Both checksum policies refuse a deciding declaration overlapping
old output, even with
`--replace-modified`. This prevents silently losing requested fairness; it
does not restrict the separate supplied-body editing contract.

The source-declaration continuation in [PlusCal source options](pluscal-source-options.md)
proves a bounded fairness/label subset and keeps requested/effective context
separate. Historical source-comment refusals and wrong-fairness captures above
remain evidence of earlier producers, rather than current positive credit.

The separately proved [macro Full scope](pluscal-macro-checksums.md) lifts the
blanket macro refusal for bounded nonrecursive substitution shapes; it does
not certify untested process/option interactions.
