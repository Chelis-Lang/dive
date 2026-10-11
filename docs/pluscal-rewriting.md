# Replacing a PlusCal translation block

`dive translate --rewrite` returns a replacement document in memory. It leaves
its input file unchanged. The default `--checksums full` derives both reference
checksums for the proved bounded sequential and finite-process AST subsets. Unproved AST shapes
return named unsupported results. Explicit `--checksums tla` supports the broader
generator scope by recording only the generated-body CRC and disabling algorithm
CRC; it never silently substitutes for full checking.

Save this P-style source as `Rewrite.tla`:

```tla
---- MODULE Rewrite ----
EXTENDS Integers
(* --algorithm A
variable x = 0;
begin
  Start: x := 1;
  Finish: skip;
end algorithm; *)
\* Keep this comment.
====
```

The equivalent C-style comment is:

```tla
(* --algorithm A {
  variable x = 0;
  { Start: x := 1; Finish: skip; }
} *)
```

Run:

```sh
dive translate Rewrite.tla --rewrite --output json > plan.json
```

On accepted generation, `result` is `replacement`; `replacement.module_source`
contains the admitted module, and `replacement.span` describes the original byte
interval. Both header fields contain derived CRCs. `algorithm_checksum_verified` and
`body_checksum_verified` are true; these mean reference CRC derivation, not source
identity, integrity certification, semantic equivalence or model proof.
`replacement.algorithm_checksum.serialized_ast` retains the reference-format
post-translation AST. `model_proved` and `exhaustive` remain false.
Translation is not model checking.
To inspect the generated document separately without overwriting the source:

```sh
mkdir -p generated
python3 -c 'import json; d=json.load(open("plan.json")); assert d["accepted"]; open("generated/Rewrite.tla", "w").write(d["replacement"]["module_source"])'
dive analyze generated/Rewrite.tla --output json
```

An unchanged existing body with a matching CRC can be replaced. A modified body,
missing checksum or disabled body checksum needs `--replace-modified`; otherwise
replacement is refused. Full requests also compare the recorded algorithm CRC
with the current derived CRC (`algorithm_matched`); missing/disabled/mismatching
values require the same grant. That comparison establishes CRC equality, not
source identity. With `--checksums tla`, old algorithm values remain unverified
and are explicitly disabled. Unsupported generation
or enclosing-module rejection returns no replacement. `--write` is unsupported;
there is no implicit backup or file overwrite.

Both source syntaxes use the same generator support boundaries as
[PlusCal translation](pluscal-normalization.md) and
[process/procedure fairness](pluscal-procedure-fairness.md). This editor does not
expand that scope. It refuses multiple algorithm comments, duplicate/reversed
sections, unproved marker placement and marker-like text outside a proved line
comment. All spans are UTF-8 byte offsets. The default source and output budget is 4 MiB;
Rust callers can configure the editor budget. AST serialization has a separate
4 MiB hard ceiling and honors lower generator output budgets. Defaults preserve surrounding bytes and use the first
source newline style for generated lines. `--compat=tlc` normalizes LF, but the
partial checksum policy is explicitly reported as `tlc+overrides`. Full requests
retain the selected profile but claim only the implemented AST scope, not full
PlusCal translation or formatting parity.

Full serialization supports sequential declarations, default initialization,
assignment (including parallel/selectors), skip/goto, await/assert/print, if/else,
either, single-binding with and while, including proved C block wrappers and
label normalization. Nested `elsif`, multiple `with` binders, multiline
expressions and normalized ASCII junction columns have proved raw CRC
derivation. Bounded sequential non-junction multiline generation is admitted;
unproved junction, binder, macro and process layouts remain named refusals.
[the expression guide](pluscal-checksum-expressions.md) gives executable C/P
and Rust examples and precise newline/column/refusal limits. The separately
measured Unicode expression controls include `∧`, `≤`, `≠` and `∈`. Definitions,
macros, procedures, scoped name fixes, branch-internal labels,
labelled C wrappers and unproved
proof-step-shaped expression tokens are named refusals. Use `--checksums tla`
explicitly when generation is supported but full serialization is not; that
policy disables rather than invents the missing checksum.

Rust callers can review the plan without disk side effects:

```rust
use dive_api::{RewriteOptions, RewriteRequest, rewrite_source};
let source = "---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n====\n";
let request = RewriteRequest {
    editing: RewriteOptions::default(),
    ..RewriteRequest::default()
};
let report = rewrite_source("R.tla", source, &request);
assert!(report.accepted());
assert_eq!(report.to_json()["model_proved"], false);
assert_eq!(report.to_json()["algorithm_checksum_verified"], true);
```

`rewrite_file` performs a bounded UTF-8 read; `rewrite_source` generates and
admits the module. `dive_pluscal::rewrite::rewrite_document` is a lower-level pure
editor of a **supplied** body: its plan alone certifies neither generation nor
TLA acceptance. The body CRC is noncryptographic and omits line separators, matching
the reference. The AST CRC includes its serialized newlines. Neither checksum
is a semantic equivalence check or proof. For a `--fair algorithm`, TLC hashes
only `"fair"`: `fair_header_crc_only` is true and algorithm CRC equality cannot
detect source changes. Native full mode deliberately reproduces that limitation;
no stronger default has been approved.

Evidence lives in
[`rewrite-acceptance`](../crates/dive-pluscal/rewrite-acceptance/README.md).
Exact primary bytes from supplied primary-body controls are separate from native
emitted bytes/admission. That earlier study retains all three
translator-success/SANY-invalid primary artifacts uncredited; the separate AST
study accounts for its own complete selection and invalid-reference outputs. The pinned fair-algorithm checksum bug
[D-020](tlc-differences.md#d-020-fair-algorithm-checksums-do-not-detect-source-edits)
is recorded separately: no corrected algorithm-checksum default is implemented.
The separate [AST study](../crates/dive-pluscal/ast-checksum-acceptance/README.md)
compares native serialized bytes with pinned parsed/name-fixed/post-translation
captures and independently checks admitted full documents with SANY and Validator.
Exact generator pretty-print bytes, file writing, mapping formats and other AST
shapes remain pending under [ADR 0047](decisions/0047-bounded-translation-block-editing.md).

## Syntax scope and reference checks

Generation preserves the reference expression scope, including parentheses
around a `CASE` assignment and the explicit `TRUE` conjunct for an empty
`else`. SANY acceptance and matching checksums alone do not prove that scope:
regression controls compare the complete generated module syntax tree with
the pinned translator, retaining parentheses, primes and operator grouping.
The original failed comparisons remain in the integration evidence.

[Conditional and expression-layout checksum support](pluscal-checksum-expressions.md)
describes the admitted nested `elsif` and multiple `with` forms. Multiline
checksum serialization and sequential non-junction multiline artifact generation
are available in the proved subset. Multiline junction, macro, process and
procedure layouts still return named unsupported results. See the
[multiline guide](pluscal-multiline.md) for the precise boundaries.

These checks do not certify full PlusCal parity, source integrity, model
correctness or byte-for-byte generator formatting.

## Prior combined publication checks

The [current source-bound receipt](../conformance/probes/native-parity-integration/current-controls/full-checksum-runtime-publication/verification.json) records the corrected combined
runtime: 982 workspace tests, zero skipped, all eight local gate stages, the
21-model safety gate and 16 ValidationTestSuite smoke models at 1/4/8 workers.
The validation smoke controls also retain CI's existing 30-second per-model
bound. Local timings were measured under shared load and are not performance
claims.

Fresh Full controls account for the complete 216-row base selection and
320-row nested/layout selection in both profiles. Every admitted artifact
passes native enclosing-module analysis, pinned SANY, pinned Validator and an
exact complete nontrivia syntax-tree comparison preserving parentheses, primes
and operator grouping: 328 base pairs and
172 nested pairs. Both selections retain their refusals, parser rejections and
SANY-invalid primary outputs uncredited; 24 source/body/header corruptions keep
the measured Validator dispositions. These are separate selections, not a
claim of 536 unique specifications or full translator coverage.

The earlier 977-test producer passed SANY and Validator but failed 12 strict
syntax-tree comparisons. Its source, binaries and complete failing campaign
remain [blocked development evidence](../conformance/probes/native-parity-integration/current-controls/full-checksum-runtime-before-cst-repair/verification.json).
The repair preserves explicit TRUE for an empty ELSE and encloses CASE values
according to the pinned translator's first-line parenthesis rule. Passing CRCs
or parser acceptance never excuse a changed expression scope.

The runnable Full guide passed 12 fresh CLI checks and its exact Cargo-owned
Rust fence; the current user-manual example passed four profile/policy controls.
All kept their input files unchanged and their nonproof fields false. Historical
BodyOnly, quota, forgery, state-count and witness campaigns remain bound to their
original producers. This publication adds no formatter, filesystem-write,
mapping-format or full-family certification.

Bounded sequential non-junction multiline generation and its executable CLI/API
examples are documented in [the multiline guide](pluscal-multiline.md). The older
raw multiline checksum captures remain separate from this new admission evidence.

Finite processes with declared fairness now have a separately proved Full
serialization contract; see [runnable process examples](pluscal-process-checksums.md).
Unproved process fairness/automatic-label options and scoped call/name contexts
remain Full refusals. CRC equality is still neither integrity nor model proof.

Source `PlusCal options (...)` declarations currently return a named unsupported
result before native generation or algorithm CRC derivation. Use supported
explicit CLI/API settings. This refusal prevents silently dropping source fairness
or label options; it does not change supplied-body editing's separate nonproof
contract. Evidence: `crates/dive-pluscal/source-option-acceptance/`.
