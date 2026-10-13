# PlusCal translation

This is the single guide to dive's native PlusCal translator. The source parser
is described in [pluscal-parser.md](pluscal-parser.md), and the user-level
walkthrough is in [user/pluscal.md](user/pluscal.md). The design decision is
[ADR 0084](decisions/0084-pluscal-translation.md).

## Usage

```sh
dive translate FILE.tla --output json            # in-memory translation
dive translate FILE.tla --rewrite --output json  # replacement module with checksums
```

`dive translate` reads one C- or P-syntax algorithm from a module comment and
returns an in-memory artifact. It never writes the source file (`--write` is
unsupported). The generated enclosing module must pass native syntax, name and
level analysis before it is returned. The JSON report always has
`model_proved: false` and `exhaustive: false`: translation is not model checking.
Save `module_source` yourself and check it as a separate model.

Rust callers use `dive_pluscal::translate` / `translate_at` for a generated body,
and `dive_api::translate_source`, `translate_file` and `rewrite_source` for a whole
module. `TranslationRequest` and `RewriteRequest` carry the source, generation and
module-resolution limits. `TranslationReport` provides `accepted()`, `exit_code()`
and `to_json()`. Source maps are UTF-8 byte associations, not TLC's map file.

Exit codes are 0 (artifact), 1 (rejected), 3 (unsupported or over a bound) and
4 (internal error). They are translator outcomes, not model-checking verdicts.
The default and `--compat=tlc` profiles generate the same supported translation.

## Supported constructs

Both current syntaxes (C and P) are supported for everything below.

- **Sequential algorithms:** initialized or finite-membership variables, whole,
  field and index assignment, simultaneous assignment, `if`/`elsif`/`else`,
  `either`, non-shadowing `with` (including multiple binders), `while`, `goto`
  to an existing label or `Done`, `await`, `print`, `assert` and `skip`.
  Sequential read-after-write priming follows the pinned translator. Assertion
  locations point to the keyword.
- **Normalization:** automatic labels when a sequential algorithm has none,
  default model-value initialization, label modifiers, termination stuttering and
  program-counter omission for the eligible infinite loop. Generated labels are
  reported separately; the source AST is preserved.
- **Multiline expressions:** bounded multiline expressions whose grouping does
  not depend on indentation (arithmetic, tuples, records, `IF`/`CASE`, selectors).
  Multiline junction lists, macro substitution into multiline text and TLA
  binders in multiline context are refused.
- **Macros:** bounded nonrecursive macros, expanded simultaneously before labels
  and process actions are generated.
- **`define` helpers:** global pure helpers, including `LOCAL` helpers and TLA
  function definitions. Name, cycle, action-level and temporal-level helpers are
  refused.
- **Processes:** one or more finite process blocks with literal IDs or finite
  membership sets, with process-local state (indexed by `self` for process sets),
  labels and weak or strong fairness. The modifier after a label's colon is
  written `Entry: +`.
- **Procedures:** explicitly labelled procedures with disjoint names, saved
  parameter and local frames, simultaneous actuals, nested calls, self and mutual
  recursion, and adjacent call/return and call/goto forms. Finite processes may
  call shared procedures; each process ID owns its own stack, parameters, locals
  and program counter. Procedure actions are generated once. Fair processes that
  call procedures produce the pinned `Spec`.
- **Options:** `--label`, `--label-root ROOT`, `-nof`, `-wf`, `-sf` and
  `-wfNext`. A supported `PlusCal options (...)` declaration in the file is also
  honoured: the first declaration the reference recognizes decides, and source
  fairness and label options override the requested ones. Reports keep both the
  requested and effective settings, with the declaration's byte span. Unknown,
  malformed or conflicting declarations return no artifact.

## Rewriting and checksums

`--rewrite` returns a replacement document in JSON (`replacement.module_source`
and the original byte `span`). The default `--checksums full` derives both
reference checksums: the algorithm AST CRC and the generated-body CRC. Full
checksums cover sequential algorithms, multiline expressions, macros, `define`
helpers, finite processes with their options, and sequential, nested, `elsif`,
acyclic and recursive procedure headers, plus one finite process calling one
procedure. Shapes outside that scope return a named unsupported result.
`--checksums tla` explicitly records only the body CRC and disables the algorithm
CRC (reported as `tlc+overrides`); it is never a silent fallback.

An unchanged existing body with a matching CRC is replaced. A modified body, a
missing or disabled checksum, or an algorithm CRC mismatch needs
`--replace-modified`. A source-option declaration that overlaps old generated
output is refused even with that flag, so requested fairness is never lost. The
editor refuses multiple algorithm comments, duplicate or reversed sections, and
marker-like text outside a proved line comment. A matching checksum is a CRC
equality, not a proof of source identity or of the model.

## Limits

Default source bounds are 1 MiB, 100,000 tokens, 50,000 nodes and depth 128.
Hard ceilings are 4 MiB and depth 256. The translator caps 10,000 actions and
4 MiB of output, and the editor and AST serializer each have a 4 MiB budget.
No partial output is returned on a bound or an error.

## Known refusals and TLC differences

Named refusals remain for general scoped renaming and generated-name
collisions, shadowing binders, unproved branch-internal label placement,
historical translator flags, exact formatter output, filesystem writes and TLC
map-file parity.

The pinned translator reports success for variables named `Next` or `vars`, and
for some `with` shadowing and `define` outputs, but SANY then rejects the module.
dive refuses those inputs before producing an artifact, so an invalid module is
never returned as accepted.

## Tests

Translator fixtures (inputs and expected pinned-translator output) live under
`crates/dive-pluscal/tests/fixtures/`. Bulky oracle captures are regenerated from
the pinned translator, not committed.

## Corpus measurement

`cargo xtask pluscal-corpus --out DIR --dive BIN` translates every selected PlusCal
input in the pinned Examples and tlaplus test corpora with native translation and
`pcal.trans`. It verifies the default jar and host JRE archive hashes, module
libraries and corpus revisions before running. Missing, modified or unreadable
pinned inputs fail the run; other cached runtime/library revisions are ignored.
Reports and scratch artifacts stay under `--out`.

```sh
cargo xtask pluscal-corpus --help
```

Help succeeds without loading pins or starting a translator. `--dive BIN` may be
relative to the invocation directory; it is resolved before entering a module's
scratch directory. `--cache DIR` defaults to `DIVE_CACHE_DIR` or `~/.cache/dive`;
`--out DIR` defaults to `<target-dir>/pluscal-corpus`, and the default binary is
`<target-dir>/release/dive`. `--filter TEXT` selects relative corpus paths by
substring. Translation and check timeouts default to 60 and 120 seconds;
`--timeout` and `--check-timeout` accept unsigned whole seconds, including zero.
Runs are sequential. These timeouts do not claim a whole-process-tree watchdog
or a total memory/output budget. A run clears its `--out/work` directory, so use
a directory owned by that measurement. `--label`, profile and jobs options are
not implemented by this helper; inherited `DIVE_COMPAT` affects native children.

`--check` runs **dive on both translations** when a same-named configuration
exists. Matching native checks support translation equivalence; they are not
independent TLC model parity. Matching refusals/resource failures and absent
reports earn no model agreement. The release milestone separately compares
`pcal.trans` plus TLC with native translation/check and independently validates
counterexamples. See [ADR 0089](decisions/0089-pluscal-measurement-inputs.md).

The measurement tool credits only recognized, well-formed check outcomes with
complete numeric counts when exploration is required, a claimed TLC class, and
a process exit consistent with the verdict. Class admission checks an exact
code/name pair, its legal TLC exit and its allowed native verdict; a recognized
invariant class cannot stand in for a model, frontend or evaluation error.
The reported class must match the deciding class for non-evaluation results.
Evaluation replay may preserve a first evaluator error or a documented site
wrapper; unrelated terminal/model classes cannot stand in for that report.
Only the actual frontend rejection schema may omit the reported code.
The bounded table is `xtask/src/pluscal_corpus/check_protocol.rs`, with its
source inventory documented in `docs/provenance.md`. Unknown or incomplete JSON,
unclaimed classes, refusals and resource failures never count as agreement.
Valid failures before exploration may have null statistics. These are still
comparisons of native checks of two translations, not independent TLC parity.

Credited reports must explicitly include `statistics`, even when its value is
null for a startup failure. Missing statistics are malformed, rather than an
alias for that explicit null. Non-null statistics require unsigned generated,
distinct and depth values, with generated at least distinct. Empty Init may
legally leave both state counters zero; depth remains the observed numeric value,
without a counter-dependent floor or bound imposed by the measurement tool.

Focused tests retain the seven report/scorer and three pin controls, plus actual
native report envelopes observed in both profiles. They reject success reported
as frontend rejection, invariant failure reported as success, and model failure
reported as invariant failure. The merged action-composition-off control retains
deciding 2103 / reported 1000 and its legal native/TLC exits. These are protocol
admission checks, not independent model verdicts. Option/discovery tests exercise
relative executables, child timeouts, deterministic filtering and failure on
unreadable inputs without silently reducing selection.
The exact counter and statistics-omission regressions exercise matching-invalid
and either-sided-invalid comparisons while retaining the legal startup nulls,
frontend reported-code omission and action-composition replay envelope. Genuine
empty-Init controls retain the native zero/zero/depth0 observations; a separate
synthetic zero/zero/depth1 envelope checks the protocol boundary without claiming
that native or a new oracle run observed it.
