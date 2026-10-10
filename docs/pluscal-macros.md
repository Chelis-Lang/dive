# Native PlusCal macros

`dive translate FILE.tla --output json` now expands bounded macros in both C/P
syntaxes before automatic labels and finite process actions are generated:

Save this Pascal-style source as `MacroExample.tla`:

```tla
---- MODULE MacroExample ----
EXTENDS Integers, TLC
(* --algorithm A
variable x = 0;
macro Set(v, a) begin
  v := a;
end macro;
begin
  Start: Set(x, 1 + 2);
  Finish: skip;
end algorithm; *)
====
```

The equivalent C-style source, saved as `MacroExampleC.tla`, is:

```tla
---- MODULE MacroExampleC ----
EXTENDS Integers, TLC
(* --algorithm A {
  variable x = 0;
  macro Set(v, a) { v := a; };
  { Start: Set(x, 1 + 2); Finish: skip; }
} *)
====
```

Run `dive translate MacroExample.tla --output json` (or the C-style filename).
The document reports `result: "artifact"`, `accepted: true`, `exit_code: 0`,
`model_proved: false` and `exhaustive: false`. Its `module_source` is the complete
validated module; its `artifact.tla` contains an assignment `x' = 1 + 2` in the
`Start` action. Input files are not rewritten. Rust callers use
`dive_api::translate_source` or `translate_file` with `TranslationRequest`.

`--compat=tlc` selects the global compatibility profile; admitted macro expansion
is shared by both profiles. `--label` enables missing-label insertion and
`--label-root PREFIX` selects the inserted names. Existing process fairness
options `-nof`, `-wf`, `-sf` and `-wfNext` apply after expansion; explicit source
`fair`/`fair +` declarations and `L: +`/`L: -` modifiers retain their measured
precedence. These options do not admit unproved namespace/procedure paths.

Actuals are substituted simultaneously. Nested calls may reference only an
earlier declaration, as in the pinned translator. Assignment actuals support
scalar names, indexed elements and record fields; selector tails are prepended
rather than discarded. Macro `if`, `either`, `while`, `with`, `await`, `assert`,
`print`, parallel assignment and `goto` are expanded into the existing owned
control-flow normalization. Supported process bodies retain their own locals,
labels and fairness metadata. Expansion cannot hide foreign locals or an unbound
`self` in constant-ID process bodies; these paths remain explicit refusals.

The source AST remains source-mapped and unchanged. Generated statements use
original call spans; assertion messages and automatically inserted labels retain
both the definition and call positions. A source error cannot expose an internal
expanded-expression offset.

## Lexical substitution and invalid reference outputs

The translator does not implement conventional capture avoidance. For example:

```tla
macro Set(v, a) begin
  with a = 3 do v := a; end with;
end macro;
```

Calling `Set(x, 2)` generates a `LET a == 3 IN` scope but assigns `x' = 2`.
Both native profiles reproduce the measured lexical behavior. General TLA+ `LET`,
quantifier and function/set binders remain named refusals until their complete
rewriting contracts are proved.

For a rejection control, change `Set(x, 1 + 2)` to `Set(x, Missing)` without
defining `Missing`. Raw generation can produce the same text as the reference,
but public translation returns `result: "rejected"`, `accepted: false`,
`exit_code: 1` at `artifact-analysis`, with no artifact/module source. A bad arity
or recursion returns a source/normalization rejection; an unproved binder or
procedure interaction returns `result: "unsupported"` and exit 3. No case
returns a proof claim.

The original 92-row selection includes six translator-success/SANY-invalid
outputs: C/P formal substitution in set binders, compound assignment targets,
and simultaneous actuals referencing an undefined name. None is counted as an
admitted artifact. Raw generator equality is diagnostic evidence only. Public
CLI/API admission checks the enclosing module and returns rejection/unsupported,
with no artifact or proof. Further scope/process rows preserve the same boundary.
Duplicate formal parameters crash the pinned translator's substitution assertion
on two original rows; native returns a named refusal, never a fabricated match.

The pinned PlusCal string scanner admits printable ASCII and supported escaped
characters. Raw `"λ"`, `"é"` and an embedded control character are rejected even
though related TLA+ lexical paths can accept wider text. The first native
comparison that overaccepted `"λ"` is retained; native PlusCal source lexing now
matches the scanner boundary. Unicode in comments remains supported.

## Evidence

The complete original, extra, corrected-process scope, lexical and source-origin selections live under
`crates/dive-pluscal/macro-acceptance`. The original malformed C-local declarations
and modifier-before-colon sources remain visible; corrected positive controls
are separate inputs. Primary `pcal.trans` runs are unmodified, and independent
SANY and normalized-AST adapter stages are recorded separately. The adapter is a
development probe; native runtime never launches Java.

The active selection accounts for **222** rows: **134** admitted token/CST/SANY
artifacts, **48** matching source rejections, **30** named native refusals,
**eight** invalid-reference dispositions and **two** primary crashes. All **27**
SANY-invalid primary outputs remain uncredited; **54** public CLI and **54**
direct API checks reject them in both profiles. The generated-model study covers
**132** finite models at 1/4/8 workers in fingerprint/exact stores (**792** width/store
checks), **25** independently valid witnesses and **100** rejected mutants.
All **44** complete-document comparator corruptions are rejected. These are
curated controls, not full family certification.

Active reports retain every row and keep grammar rejection, native refusal,
oracle crash and invalid reference output separate. Artifact matches require
significant tokens, source/process/action/label metadata and nonproof dispositions.
All admitted artifacts get full enclosing-module operator CST and pinned SANY
checks. Curated generated models compare clean TLC final counts/verdict/depth at
1/4/8 workers in fingerprint/exact stores. Independent trace validation checks
real witnesses and rejects mutated ones. Corrupted complete report documents
must be disagreements, including raw-invalid diagnostic rows.

Reproduce by running `make_cases.py`, `make_extra.py`, `make_process.py` and
`make_scope.py`, `make_lexical.py` and `make_origins.py`; capture new oracle evidence into a separate directory with
`capture.py --reason ...`, then use `compare.py`, `validate.py`,
`check-admission.py` and `test-comparison.py`. Existing fixture directories and
source selections are immutable. Reports contain script/input/oracle/binary
hashes; `delivery.json` and `verify.py` bind the final source/gate evidence.

## Limits

Expansion charges source-dependent node/token/text amplification before retaining
it, with hard ceilings of 200,000 expanded nodes, 400,000 charged tokens and
4 MiB charged text. Every token append checks its running text quota before cloning large actuals.
Source parser limits can lower these budgets (four times the
configured source budget), and existing normalized action/output limits still
apply globally. A bound returns `DIVE-P0040` and no artifact.

Recursive/forward macro calls, labels inside macro bodies and bad arities follow
reference rejection. Duplicate formals, multiline substitution, unproved TLA+
binders, procedure interactions, namespace renaming and existing unproved
process domains remain explicit refusals. File rewrite/checksum, historical
options, exact pretty width and mapping-format parity are pending. A translated
artifact is never a model proof; F-034/F-035 remain partial and audit-required.
See [ADR 0039](decisions/0039-pluscal-macro-substitution.md).

Earlier combined-source verification, before ordered temporal execution, repeats the complete 222-row selection,
134 full-CST/SANY artifacts, 132 finite models (792 width/store runs), 25 valid
witnesses and 100 rejected mutants. All 54 CLI invalid-artifact controls and
44 document corruptions still reject. The current implementation passed the
830-test workspace gate and all 21 CM1 plus 16 ValidationTestSuite models at
1/4/8 workers; four macro CLI examples and three module-guide commands passed.
[The current receipt](../conformance/probes/native-parity-integration/current-controls/macros-integration/verification.json)
records the binary/source identities separately from the immutable original
macro delivery; none of these controls certifies the full translator family.

A separate macro-plus-ordered-execution source passed all 846 workspace tests,
20 Python tests and 11 source checkers. Its fresh release repeats the complete
222-row census, 134 enclosing CST/SANY artifacts, 132 finite models and
792 width/store controls, with 25 independently valid witnesses and 100 rejected
mutants. CM1's 21 models and all 16 ValidationTestSuite models fully agree at
1/4/8 workers; four macro guide CLI examples pass.
[This source-qualified receipt](../conformance/probes/native-parity-integration/current-controls/macros-ordered-execution-integration/verification.json)
preserves the prior source's results and precedes the procedure-graph increment;
none of these bounded studies certifies the entire translator.
