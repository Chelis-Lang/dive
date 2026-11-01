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
returns an in-memory artifact. An existing translation block is replaced at its
original position, preserving user definitions before and after it. Plain
translation does not retain stale checksum headers; use `--rewrite` to request
checksum interoperability. It never writes the source file (`--write` is
unsupported). The generated enclosing module must pass native syntax, name and
level analysis before it is returned. Schema2 reports this artifact admission
without model-proof or exploration fields: translation is not model checking.
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

- **Sequential algorithms:** variable initialization, whole/field/index and
  simultaneous assignment, `if`/`elsif`/`else`, `either`, `with` (including
  multiple binders), `while`, `goto`, `await`, `print`, `assert` and `skip`.
  Later statements in one action read the correctly primed state.
- **Normalization:** automatic sequential labels, explicit process label
  insertion, labels inside branches, default model-value initialization,
  termination stuttering and eligible program-counter omission. Inserted labels
  retain source positions and macro call/definition associations.
- **Expressions and macros:** bounded multiline junctions and substitutions,
  TLA binders, record labels, and macros defined in source order. Expansion
  happens before label insertion and can appear in procedures and processes.
- **Definitions and names:** `define` sections and scoped symbol disambiguation
  follow the reference translator. Generated operator collisions still refuse.
  Whole-module analysis checks the resulting names and expression levels.
- **Processes:** constant IDs and membership expressions, including symbolic
  domains, scalar or indexed local storage, and weak/strong fairness with label
  modifiers. Translation does not evaluate a domain or prove it finite or
  disjoint; model checking is a separate operation.
- **Procedures:** saved parameter/local frames, simultaneous actuals, nested,
  self and mutual recursion, adjacent call/return and call/goto, and shared
  process-owned procedures. Each procedure's actions are emitted once.
- **Options:** `--label`/`-label`, `--label-root ROOT`/`-labelRoot ROOT`,
  `-reportLabels`, `-nof`, `-wf`, `-sf`, `-wfNext`, `-termination`,
  `-noDoneDisjunct`, `-lineWidth N` and the supported `-version V` shapes below.
  A supported `PlusCal options (...)` declaration in the file is also
  honoured: the first declaration the reference recognizes decides, and source
  fairness and label options override the requested ones. Reports keep both the
  requested and effective settings, with the declaration's byte span. Unknown,
  malformed or conflicting declarations return no artifact.

## Translator options and artifact policies

`-termination` adds `PROPERTY Termination` to the returned configuration
requirements and supplies weak fairness only when fairness is unspecified.
Explicit `-nof`, `-sf` and `-wfNext` retain their meanings; source fairness
can override CLI fairness. The flag does not prove termination or control whether
its operator is generated. `-noDoneDisjunct` removes the terminating stutter
action/disjunct while retaining the independently generated Termination operator.
These options default to false, matching the pinned translator.

`-lineWidth N` accepts an i32 integer at least 60 (default 78). It selects the
reference wrapping decisions, rather than limiting every generated line to N
characters. The output budget still applies for `2147483647`; width does not
allocate that many spaces. Source options accept only dashed `-lineWidth`.
Both dashed and bare `termination`, `noDoneDisjunct` and `version` are legal.

`-version V` uses the reference decimal-thousandths conversion, not semantic
version ordering. The actual current default weight is 1903. Explicit `1.12`
is weight 1120 and differs from the default despite matching the printed
translator version. Before 1.5 the sequential translator retains `pc` and
termination stuttering even on a single-label infinite loop. Exactly 1.5 permits
sequential `fair` (or `fair +`) before `begin`; this selects weak fairness without
selecting the special `--fair` header CRC. Pre-1.5 processes/procedures currently
return named unsupported results because their historical AST fields and
translation have no consumed primary controls in this change. Invalid or later
versions return no artifact. `TranslationOptions.version` selects these settings;
standalone parsing exposes `Options.input_version` for the historical grammar.
Complete-module translation and both rewrite checksum policies use the same
resolved grammar to select the first algorithm inside its comment; following prose
or a second algorithm is outside that consumed prefix. The original AST operation
uses that shared selector with its effective grammar and resource bounds.

`-reportLabels` enables insertion and prints added names and original positions
to stderr in human mode, leaving stdout as module source. JSON keeps the
structured `inserted_labels` and reporting preference. Plain `-label` inserts
without that transcript. `-reportLabels` remains illegal in source declarations.
`-help` displays the supported and unfinished options; unknown CLI flags retain
normal dive errors rather than triggering the pinned trailing-flag help quirk.

`-noold` and `-nocfg` express backup/configuration preferences in the returned
`filesystem_plan`. The same dashed spellings are accepted in source options.
`writes_performed` remains false: these preferences do not add file-writing
support. `configuration_requirements` retains initialization and termination
requirements even when `configuration_requested` is false. The plan does not
claim to merge an existing cfg or reproduce replacement of an existing `.old`.

`-unixEOL` is CLI-only and records `unix_eol_requested`. On the pinned JRE,
measured forced-CRLF output retains CRLF both with and without this flag; setting
`line.separator` does not override the already established writer separator.
The native body is canonical LF. Default rewriting preserves source newline
bytes; compatibility rewriting retains its existing LF normalization policy.
Replacement body separators follow the first physical source break (LF, CRLF or
CR) unless normalized. `filesystem_plan.body_newlines` reports the emitted
replacement's `lf`, `crlf` or `cr` separator. Action and macro call-site mappings
retain literal original UTF-8 byte intervals across CR/LF/CRLF and mixed endings,
including expanded tab columns and supplementary Unicode characters. This does
not broaden the separate original-AST serializer's bare-CR boundary.
The flag does not promise a different newline policy. Body CRC concatenates
lines without newline bytes, whereas AST CRC includes the serializer's LF bytes.

For example, this checked sequential input supports the semantic flags in both
profiles:

```sh
dive translate docs/user/examples/Algorithm.tla -termination -noDoneDisjunct -lineWidth 60 --rewrite --output json
```

Rust callers set `TranslationRequest.limits` (a `TranslationOptions`), including
`check_termination`, `no_done_disjunct`, `line_width`, `version`, `report_labels`,
`no_old`, `no_configuration` and `unix_eol`. The JSON `translation_options`
reports resolved settings that drive generation and any available exact checksum.

## Unfinished flag semantics

`-debug` adds the pinned `(*line, column*)` AST prefixes and their indentation.
Exact debug checksums remain available for the supported procedure-free
assignment/skip/goto/await/assert/print, If/Either/While, With, C block and macro
shapes. Procedure
algorithm CRCs are unavailable after header-transfer retirement, including flat
procedural headers. That metadata boundary does not gate real generation. Broader
original-stage and debug-header compatibility remains unfinished; original
artifact availability does not imply a post-generation CRC.
Source `debug` remains illegal, as in the pinned option parser.

`dive translate MODULE.tla --ast [-debug]` is an explicit in-memory original AST
operation, available in both profiles. Human output is the `AST.tla` contents;
JSON returns `ast_artifact` with `artifact_available: true`, `accepted: false`,
`generated_module_accepted: false` and `writes_performed: false`. Native exit 0
means original bytes are available; no enclosing translated-module analysis or
model proof was performed. The operation runs parsing, macro/label normalization
and serialization before name fixing and generation. The wrapper retains the
parser's sequential fair/legacy-fair precedence over CLI fairness. Its semantic serializer
now preserves modern procedure and process parse-time call/modifier vectors,
including the last sequential procedure/main vector alias. It expands macros
using the shared token/substitution kernel and serializes source locations, grouped
labels and compound nodes before generation. Source-finite recursive calls stay
as call nodes; no calls are executed or unrolled. Pre-1.5 procedure/process fields
and full original-stage coverage remain unfinished. It can return original names
that later translation must rename or reject. No header CRC is advertised for
this earlier stage.

```sh
dive translate docs/user/examples/AstExample.tla --ast --output json
dive translate docs/user/examples/MultipleCallExample.tla --ast -debug --output json
dive translate docs/user/examples/AstExample.tla -debug --rewrite --output json
```

Rust callers use `dive_pluscal::original_ast_at` for a selected algorithm, or
`dive_api::original_ast_source` / `original_ast_file` for a module. The returned
`OriginalAstReport::artifact_available()` is deliberately separate from
`TranslationReport::accepted()`. The module operation resolves source/request grammar
before refining the actual first algorithm prefix; it does not recover a malformed
first algorithm by selecting a later one. The explicit-region operation keeps
strict end-of-input checks.

`-writeAST` still requests a filesystem operation and remains named unsupported.
The retained pinned plain/debug controls write `AST.tla` and exit 255 without
translating the input. Pinned `trans.performTranslation` calls `WriteAST`, then
returns null; `trans.run` maps that null to its error status before normal module,
backup or cfg writes. A failed write has separate handling. Those observations do
not turn the native in-memory artifact operation into filesystem or exit parity,
and do not generalize one retained successful write to every input shape.

`-spec`, `-myspec`, `-spec2` and `-myspec2` select specification-backed translator
backends, rather than fairness modes or aliases for native translation. Corrected
local-spec controls generate Init with x=42 from an algorithm initialized to x=0
and pass pinned SANY. The public `specification_backend` request and CLI options
remain named unsupported until the actual backend is implemented. Bare/dashed
source `spec` and `myspec` are recognized refusals; source spec2/myspec2 remain
illegal. These refusals are unfinished work, not completion of all translator flags.

The pinned translator ignores the returned status of source option processing:
illegal bare `lineWidth` prints an unrecoverable option error but can still exit 0
with a default-width translation accepted by SANY. Dive retains atomic refusal
of such source declarations. No partial-option success or compatibility policy
change is introduced here.

## Rewriting and checksums

`--rewrite` returns a replacement document in JSON (`replacement.module_source`
and its original byte `span`). One best-effort policy always writes the actual
line-concatenation body CRC. Exact algorithm CRCs remain available for supported
procedure-free shapes. When algorithm serialization is unavailable, the header
uses `chksum(pcal) \in STRING` and keeps `chksum(tla) = "..."`. The pinned validator
recognizes both clauses and still warns about edited TLA. Physically omitting the
pcal clause loses that body check. A warning explains unavailable algorithm CRCs;
no guessed CRC, serialized-AST proof dump or model claim is returned. Exhausting
only the checksum serializer budget is also nonfatal once real generation and
module analysis succeed. Bare CR source remains admitted by the real
generator with physical-line semantics matching the pinned reader; its legacy
direct exact checksum operation still returns a named Unsupported result. Both
policy aliases and profiles preserve real body CRCs and that warning.

`--checksums full` and `--checksums tla` are compatibility spellings for this
same behavior. Rust uses `ChecksumPolicy::BestEffort`; the old policy names remain
aliases. This intentionally changes the old forced body-only and Full-refusal
contracts in both profiles. It does not claim byte identity with an unmodified
primary header where dive disables an unavailable CRC. A primary route whose
input explicitly disables the pcal value preserves that value; those controlled
reference results stay separate from the original primary translation.

Procedure translation, recursion and macros keep their real generator semantics.
The Java shared-header transfer emulation is retired. Direct exact procedure CRC
requests remain unsupported, while rewriting and `-debug` generation proceed
without that CRC. Action/temporal DEFINE helpers accepted by complete-module
analysis can likewise rewrite with only the real body CRC. Original-AST data is
a separate operation and is never substituted for unknown post-generation CRCs.

A changed algorithm with an unchanged verified old body regenerates normally.
In the default profile a modified body, or a missing/disabled old body CRC,
requires `--replace-modified`. `--compat=tlc` automatically returns the replacement
through the pinned ordinary pcal Noop overwrite policy. The explicit flag is the
same counterpart in the default profile; no source file is written. Requested
policy is `requested_editing`, resolved policy is `editing`, and the old body
CRC/match remain in `replacement.selection.checksum`. Algorithm CRC mismatch or
absence is not an overwrite gate. Source options overlapping old generated text,
ambiguous/reversed markers, actual source errors, invalid final modules and real
generation/rewrite bounds still refuse with their existing diagnostics.

Translation, rewrite and original-AST JSON use report schema 3. Real accepted/status,
analysis, replacement/artifact bytes and requested/effective options remain.
`source_options` identifies the deciding declaration. Proof-only model/exhaustive/
pending/verified fields and duplicate `option_context` dumps are removed. Rewrite
`replacement.algorithm_checksum` is a CRC string or null, with any unavailability
in `replacement.warnings`; `translation_checksum` is always the real body CRC.
An admitted translation is not a model-checking result. The corpus runner admits
the actual accepted replacement and module-analysis fields; retired proof fields
do not establish translation or model acceptance.

The retained six procedure-family manifests contain740 historical primary rows.
688 row instances have genuine source inputs; recursive/acyclic manifests use
explicit sibling input lineage recorded in
`crates/dive-pluscal/tests/fixtures/procedure-input-lineage.json`.52 historical
instances (26 unique IDs,14 with successful primary translation facts) lack
original inputs and remain unexercised input-closure debt. They are not native
passes, refusals or coverage. The485 orphan procedure-header AST captures are
retired with the alias serializer; source inputs, unchanged manifest facts and
active original-AST fixtures remain. This bounded fixture coverage does not
establish the fresh149 corpus, model or independent-trace obligations.

## Limits

Default source bounds are 1 MiB, 100,000 tokens, 50,000 nodes and depth 128.
Hard ceilings are 4 MiB and depth 256. The translator caps 10,000 actions and
4 MiB of output during construction, and the editor and AST serializer each have
a 4 MiB budget. The selected depth also bounds macro-expanded control flow,
before substitution and after normalization.
Macro expansion shares a pre-allocation budget across definitions, procedures
and processes: at most 20,000 expanded statements, 400,000 expanded tokens and
4 MiB of token text and substituted expression layout, reduced by the requested
parser and output limits.
No partial output is returned on a bound or an error.

## Known refusals and TLC differences

Named refusals remain for generated-operator collisions, duplicate macro formal
parameters, unsupported source grammar/layout, pre-1.5 process/procedure shapes,
compound/process-owned procedural debug transfers, procedural macro/DEFINE or
automatic/fairness-label ownership, simultaneous-assignment ordering and
identifier-renaming debug headers,
specification-backed translators, filesystem
writes and TLC map-file parity. Exact algorithm-CRC serialization has a narrower procedure-free scope;
successful translation does not require that optional CRC.

The pinned translator can write a module that SANY rejects: variables named
`Next` or `vars` collide with generated operators, and `print` needs `EXTENDS
TLC` for `PrintT`. dive's public API keeps enclosing-module syntax/name/level
analysis as a required stage, so those outputs earn no admitted-artifact credit.
Both compatibility profiles use the same rule.

The pinned translator initializes an empty sequential main body to `pc = "null"`
when procedures still require a program counter. The retained empty-main controls
compare that literal and the complete generated token stream. Multiline process
fairness uses the reference conjunction-list formatting and its width calculation.

## Tests

Translator fixtures (inputs and expected pinned-translator output) live under
`crates/dive-pluscal/tests/fixtures/`. Bulky oracle captures are regenerated from
the pinned translator, not committed. Regenerate missing consumed outputs with
`python3 -I scripts/pluscal-fixture-regen.py crates/dive-pluscal/tests/fixtures`.
The script verifies the oracle pin; commit only files read by a passing test,
with each expected output at most 100 KB.

The multiline test keeps the historical 208-row report unchanged. It compares
164 admitted artifacts with the primary module CST, serialized original AST and
algorithm CRC; the other 44 rows retain their rejection/refusal assertions.
The port adds 34 explicit admissions with generated primary modules and ASTs,
each also accepted by pinned SANY. AST controls come from the pinned
`ParseAlgorithm.getAlgorithm`, `PCalTLAGenerator.removeNameConflicts` and
`PCalTLAGenerator.translate` stages followed by the original AST's `toString`,
not the exploded tree. These stages reproduce all 130 existing AST fixtures.

`remaining_options.rs` consumes 28 primary body/algorithm-CRC controls for
termination/fairness/source overrides, noDoneDisjunct, widths 60/78/100, label
insertion/reporting, version 1.5 grammar and version-dependent pc optimization.
The compact `fixtures/remaining-options/primary.json` contains only fields read
by those tests. Whole-module API tests exercise both profiles and best-effort
replacement, artifact policies, source context and atomic refusals. CLI tests
check actual spellings, label transcripts and unchanged input files.

`original_ast.rs` consumes exact retained plain/debug `AST.tla` contents,
the debug algorithm/body CRC pair and four DEFINE original-tree exports. The
DEFINE exporter runs name fixing and generation before serializing the original
tree; it is distinct from the two pre-generation `-writeAST` captures. These
selected collision-free definitions and assignment/skip bodies retain their
original serialization through those stages; broader original-stage DEFINE
ownership is not established by these controls.
`pluscal_original_ast.rs` and CLI tests consume the same primary fixtures in both
profiles, keeping AST availability, translation admission, header CRCs and file
writes separate. These are stage controls, not a fresh corpus campaign.

`original_ast_ownership.rs` consumes 32 actual pre-generation WriteAST artifacts
for procedure/main/process vectors, source-finite recursion, nested/multiline macro
substitution, expression line gaps, and P/C compound/grouped-label locations.
Eleven retained procedure-free debug translations match algorithm CRCs, generated lines and
body CRCs, including the three labelled If/Either/While controls. If/Either shared
unlabelled vectors mutate; the measured While header is a no-mutation positive.
A separately named procedural control adds explicit EXTENDS Sequences and retains
its actual body `5cd8f98a`; the old header CRC capture is historical. Its current
rewrite explicitly disables the algorithm CRC after transfer retirement.
All twelve actual SANY runs reach semantic processing and linting of Case with
one changed-since-translation warning. The nested-macro run additionally prints
an embedded missing-label validator error because SANY does not replay `-label`.
These are generated-module semantic successes with warnings, not clean original
PlusCal checksum validation, model proofs or a full campaign.
Four invalid initial source families stay consumed as failures; corrected inputs
have separate IDs. Generated body comparisons ignore only the translation-block's
terminal separator line breaks; body CRCs retain the pinned line-byte contract.
The primary JSON fixtures retain exact original module bytes and the consumed
later-stage fields, each below 100 KB. API and CLI tests exercise both profiles,
Full replacement, no-proof reports and untouched filesystem sentinels. No AstDump
post-generation export is used as pre-generation authority for this increment.

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
a directory owned by that measurement. `--label` passes the actual `-label`
argument to both translators and records `translation_flags` in the result;
it defaults to false and never silently alters the measured source. Profile and
jobs options are not implemented by this helper; inherited `DIVE_COMPAT` affects
native children.

`--check` runs pinned TLC on the reference translation and dive on its own
translation in both semantic profiles when a same-named configuration exists.
Build `dive-conformance` beside `dive`, or select it with `--harness BIN`.
The ordinary harness classifies the clean TLC stream, attaches JSON only from a
matching separate trace pass, compares required observables and freshly validates
native counterexamples against their translated model. Both checks use identical
configuration bytes, one worker, seed zero and fingerprint zero. Registered
default-profile divergences remain separate from strict agreements. Missing
configurations, refused translations, unsupported checks, stops and unconfirmed
words earn no model agreement. All selected inputs remain in the results. Raw
children and physical status receipts remain under `--out`.
`results.json` includes `summary.accepted`; any refusal, stop, missing model
configuration, differing translation or non-agreeing check makes it false and
the command exits nonzero after writing its complete report. A comparison has
an overall deadline of four check timeouts plus 60 seconds. Its own supervisor
cancels and drains registered child groups on that deadline, including when
source binding or independent replay is still running; the outer runner waits
for supervised cleanup rather than killing that supervisor.
Historical runs that checked both translations using dive retain their narrower
diagnostic meaning. See [ADR 0089](decisions/0089-pluscal-measurement-inputs.md).

The measurement tool credits only recognized, well-formed check outcomes with
complete numeric counts when exploration is required, a claimed TLC class, and
a process exit consistent with the verdict. Class admission checks an exact
code/name pair, its legal TLC exit and its allowed native verdict; a recognized
invariant class cannot stand in for a model, frontend or evaluation error.
The reported class must match the deciding class for non-evaluation results.
Evaluation replay may preserve a first evaluator error or a documented site
wrapper; unrelated terminal/model classes cannot stand in for that report.
The current schema-3 ordinary temporal producer omits `tlc.reported` for holds
and liveness violations, typed planning errors (`TLC_EXPECTED_VALUE`) and direct
temporal-action GENERAL errors. Admission recognizes those exact temporal
phase/code/verdict shapes; a present field must match their deciding code.
Safety results retain their reported-code requirements, except frontend
rejection. Temporal binding failures, startup tautologies and certified finite
state/source-row errors retain their explicit reported code. These transport
rules do not establish a temporal result: canonical schedule/capture reading
and fresh independent validation of the actual reported word still govern
agreement. Genuine holds, violations, planning errors, startup tautologies and
finite row errors are exercised through actual producer reports in both profiles.
The shared envelope admission in `crates/dive-conformance/src/native_envelope.rs`
runs before the canonical source-aware harness reader supplies classification.
It retains the original exact class/verdict/reported-code associations; the
current helper also requires explicit JSON and physical exits to match its
requested TLC exit convention. Both production and the retained regression
controls use the same bounded admission source. Unknown or incomplete JSON,
unclaimed classes, refusals and resource failures never count as agreement.
Valid failures before exploration may have null statistics. The retained
malformed-envelope controls supplement the independent comparison reader.

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

The separate procedural Elsif fixture family retains32 genuine inputs and primary
body-CRC facts, with generator/resource and API module-admission controls. It adds
32 input rows to the688 source-closed rows above; the52 missing historical rows
remain unexercised. Its former32 header-alias AST captures are included in the485
retired orphan files. No procedure-free exact CRC or original-AST fixture is removed.

Current PlusCal overwrite policy: the default API/CLI protects modified or
unverified existing bodies, with `--replace-modified` as its explicit overwrite
counterpart. `--compat=tlc` resolves that protection to ordinary primary Noop
overwrite automatically. Pure low-level `rewrite_document` retains its caller's
explicit policy. `requested_editing` records input settings, `editing` records
effective settings, and old body CRC/match data remains observable. Marker,
source-option, generation, module-analysis and quota refusals are unchanged.
The fresh exact modified-body primary control supports its own complete output
comparison; missing/disabled header variants do not inherit header byte-identity
credit from that control.
