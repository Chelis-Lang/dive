# Native sequential PlusCal translation

`dive translate FILE.tla --compat=tlc --output json` translates one current C/P
algorithm in a module block comment. The operation returns an in-memory module;
it does not rewrite the file. The generated module must pass native TLA syntax,
name and level analysis before the API returns an artifact. The JSON document
always reports `model_proved=false` and `exhaustive=false`.

The Rust entries are `dive_pluscal::translate` / `translate_at` for a generated
body and `dive_api::translate_source` / `translate_file` for an admitted enclosing
module. `TranslationRequest` carries source/generation and module-resolution
limits. Body/source maps are native UTF-8 byte associations, not TLC's map file.
Library/module paths can be supplied on the CLI. Current proved generation is
shared by default and TLC profiles; `--compat=tlc` does not remove explicit
unsupported-feature or resource qualifications.

## Supported boundary

An explicitly labelled single sequential algorithm may declare initialized or
finite-membership variables, assign whole variables/fields/indices, perform
simultaneous assignment, branch with `if`/`elsif` or `either`, use non-shadowing
`with`, loop with `while`, jump to an existing label/`Done`, await, print, assert
and skip. Fairness declarations, labelled action order, termination stuttering,
program-counter omission for the eligible infinite loop and sequential
read-after-write priming follow the pinned translator. Assertion locations point
to the keyword, including when a label precedes it.

Named refusals cover processes, procedures/calls/returns, macros, `define`,
automatic label insertion/label fairness modifiers, unproved label placement, default initialization,
name disambiguation/generated-name collisions and shadowing binders. TLA lexical
binding constructs (including Unicode quantifiers/function binders) and multiline
expression layout are refused until scope/layout-preserving substitution is
proved. Complete module admission can reject malformed expression slices that
the source grammar retained. Existing generated translations, multiple algorithm
markers, historical flags, pretty-width, file rewriting/checksums and TLC map
format are pending. A source AST alone never certifies a translation.

Default source bounds are 1 MiB / 100,000 tokens / 50,000 nodes / depth 128;
hard source/depth ceilings are 4 MiB/256. The translator additionally caps 10,000
actions and 4 MiB output. Normalized trees are depth-checked before construction,
including folded flat `elsif` chains. Each call joins its own 16 MiB stack worker.
Diagnostic stages distinguish source, normalization, generation and enclosing
artifact analysis. No partially accepted output is returned on a bound or error.
Native exits are 0 artifact, 1 rejected, 3 unsupported/bound and 4 internal error;
these do not imply TLC model-checking verdicts or historical translator CLI parity.

## Evidence and limitations

The retained matrix includes every original 83 row, all later 149 controls and
eleven assertion/Unicode/layout/label boundary controls: **160** inputs: 46 matching generated artifacts, 55 matching rejections and 59
named refusals. Two additional valid label-modifier admission controls retain
primary/SANY acceptance and named native refusal separately. Clean primary
`pcal.trans` outputs and SANY results retain every disposition, including invalid
primary artifacts. Generated bodies are compared token-for-token; each accepted
full module additionally compares its span/trivia-free operator CST, retaining
grouping, primes, operator structure and junction nesting. All **46** admitted modules match that operator structure and pass separate
pinned SANY checks.

**16** finite generated-model controls compare complete/safety verdicts and all reported
counts against clean single-worker TLC (`-seed 0 -fp 0 -lncheck final`). Independent
trace replay accepts **four** real invariant witnesses and rejects all **12**
altered Init/Next and missing-variable states. Eight complete-document corruptions
must be rejected by the artifact comparator. Raw controls, artifacts, hashes and reports live in
`crates/dive-pluscal/translation-acceptance/`. The initial validation experiment
used incorrectly shaped trace mutations; its failed evidence is preserved in
`initial-control-shape/` and earns no agreement credit. The repaired controls
mutate each state's actual `values` object and inspect the validator disposition.

The pinned translator announces success for variables named `Next` or `vars`,
but SANY rejects the resulting module. `with x=...` shadowing and some `define`
outputs also appear in the retained invalid-artifact rows. Native generation
refuses unproved naming paths before artifact admission. This is a current
limitation, not a new name-disambiguation default. File checksums, formatting,
precise mapping and full F-034 certification remain pending.

See the [reproduction instructions](../crates/dive-pluscal/translation-acceptance/README.md)
and [ADR 0026](decisions/0026-pluscal-sequential-translation.md).
