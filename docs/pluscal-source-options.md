# PlusCal source options

A supported `PlusCal options (...)` declaration can configure native translation
and Full checksum editing. The first reference-recognized declaration in the
whole file decides the settings. Source fairness overrides the requested CLI/API
fairness; the source label/root options override those corresponding settings.
Reports keep both the requested and effective settings and original byte spans.
This changes artifact generation, not the checking profile or a model verdict.

Save this as `SourceOptions.tla`:

```tla
---- MODULE SourceOptions ----
EXTENDS Integers, TLC, Sequences
\* PlusCal options (-wf)
(* --algorithm A variables g=0;
process P \in {1,2} begin L:g:=1; E:skip; end process;
end algorithm; *)
====
```

```sh
cargo run -p dive-cli -- translate SourceOptions.tla --compat=tlc -sf --output json
cargo run -p dive-cli -- translate SourceOptions.tla --compat=tlc -sf --rewrite --output json
cargo run -p dive-api --example pluscal_source_context -- SourceOptions.tla rewrite strong tlc
```

The generated `Spec` uses weak fairness, because the source `-wf` replaces the
requested strong setting. The JSON contains `option_context.requested.fairness`
`Strong`, `option_context.effective.fairness` `Weak`, and the deciding declaration
with original argument spans. An accepted rewrite returns its in-memory
`replacement.module_source`; it does not write the input file. `model_proved`
and `exhaustive` remain false. Both profiles use the proved pinned semantics;
`--compat=tlc` does not enable unknown options or partial error application.

The equivalent C algorithm is:

```tla
(* --algorithm A { variables g=0;
process (P \in {1,2}) { L:g:=1; E:skip; } } *)
```

Supported source arguments are `-nof`, `-wf`, `-sf`, `-wfNext`, `-label`, and
`-labelRoot NAME`, and their forms without the hyphen. A declaration may combine
one fairness setting with labels/root. Empty declarations preserve requested
settings. Repeated label roots use the last root; that option does not itself
turn on automatic labels. Multiple fairness arguments, including repeats,
unknown options and malformed declarations return named `unsupported` with no
artifact or checksum. There are at most 128 argument words, at most 256 bytes per
word, and a 4 MiB whole-file scanning ceiling, alongside existing parser,
normalization and generation quotas. Roots must use a proved ASCII identifier.
Adjacent non-ASCII keyword boundaries remain unproved and refused. Bare-CR
sources can produce a body but Full AST serialization remains refused.

The reference scanner does not remove comments or quoted text. A space-separated
`PlusCal options` inside either can decide settings. `(*PlusCal` and
`\*PlusCal` do not decide them. The first declaration can appear before or after
the algorithm. A deciding declaration inside an existing translation block is
refused before masking: replacing that block would erase the context used by the
reference. The low-level supplied-body editor is a separate contract and never
admits a body or proves a model by itself.

Use the typed Rust API without a Java runtime:

```rust
use dive_api::{TranslationRequest, translate_source};
use dive_pluscal::{FairnessOption, TranslationOptions};
let source = "---- MODULE R ----\nEXTENDS Integers,TLC,Sequences\n\\* PlusCal options (-wf)\n(* --algorithm A variable g=0; process P=1 begin L:g:=1; E:skip; end process; end algorithm; *)\n====\n";
let request = TranslationRequest {
    limits: TranslationOptions { fairness: FairnessOption::Strong, ..Default::default() },
    ..Default::default()
};
let report = translate_source("R.tla", source, &request);
assert!(report.accepted());
assert_eq!(report.to_json()["translation_options"]["fairness"], "Weak");
assert_eq!(report.to_json()["model_proved"], false);
```

Checksum equality certifies the bounded reference CRC derivation, not source
identity. The `--fair algorithm` header still hashes only `fair` (D-020), and
explicit CLI-only options still need the same context in the reference Validator
(D-021). The native tool does not invent a stronger default checksum. Exact
primary formatting, filesystem writes/maps, general name renaming and unproved
macro/procedure AST serialization remain pending. Bounded nonrecursive macro
Full serialization is separately documented in [the macro guide](pluscal-macro-checksums.md).

Development selection and raw primary errors are retained in
`crates/dive-pluscal/source-option-semantics-acceptance/`; primary translation,
post-translation AST bytes/CRC, whole-module CST, SANY admission, configured
Validator, finite model checks and independent witness controls are separate
obligations. Earlier failed header parsing and the original mandatory-refusal
test are preserved. No whole translator family certification follows from this
increment. See [option/header limitations](tlc-differences.md#d-021-pluscal-checksums-need-the-generation-options)
and [Full editing](pluscal-rewriting.md).

## Executed bounded controls

The complete selection retains **752 profile rows**. Its raw stage census is 480 exact raw ast, 16 primary invalid raw serialization, 16 rejection accounted, 208 named refusal, 16 named selection refusal, 16 generation only full refused. Raw invalid artifacts and generation-only rows receive no Full/admission credit. The public campaign records 480 admitted exact cst sany validator, 16 invalid primary uncredited, 32 rejection accounted, 224 named public refusal. Every admitted row separately passed complete operator/module CST, SANY and configured/unconfigured reference Validator controls.

The executable guide has 13 CLI/API/Cargo controls. Six unchanged/corrupted checksum controls and 15 forged-report mutations passed; 496 requested-context audits preserve the actual input settings. The selected generated modules have 24 finite primary checking controls across 288 native worker/store/profile settings. Independent strict replay records 12 valid witnesses and 48 rejected mutants; any original strict refusal remains pending rather than certified. These finite safety checks do not prove generated fairness/liveness properties.

The frozen uncommitted producer passed 719 workspace tests, all other local gate stages, and its ancestor CM1 20/VTS16 at workers 1, 4 and 8. The delivery receipt retains the original producer bytes and failed campaigns; eventual committed-source equality is checked separately. Current-main integration must run its own complete gates. F-034/F-035 remain partial.
