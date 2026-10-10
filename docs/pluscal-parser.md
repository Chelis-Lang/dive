# Native PlusCal source parsing

`dive-pluscal::parse` parses standalone current C/P syntax. `parse_at` parses an
explicit region in an original module. Results retain source, tokens, typed
declarations/processes/macros/procedures, fairness, labels/modifiers and all
ordered assignments/control flow. Statements use an arena; grouping and macro
calls remain explicit. Spans are original UTF-8 bytes, with optional Unicode/tab
display positions. No Java runtime is used.

**Source grammar is not translation acceptance.** Expressions follow the pinned
delimiter/quantifier counting rules, without TLA syntax/semantic validation.
Primary and secondary probes accept `(1]`, `[1)`, `<<1}` and `[1, end]` at this
stage; those slices must be validated later. Macro expansion, labels/name checks,
full translation/checksums, historical dialects and translator options remain
pending. A separate [bounded sequential translation path](pluscal-translation.md)
now admits a single algorithm in a module comment; source acceptance alone does
not imply translation acceptance.

Default bounds: 1 MiB source, 100,000 significant tokens, 50,000 counted nodes
and nesting 128. Hard byte/depth ceilings: 4 MiB/256. Temporary lexer storage is
bounded by source size. Each call joins a scoped 16 MiB worker, independent of
its caller stack. Stable diagnostics include lexical/grammar/expression/label
errors, resources (`DIVE-P0040`) and unexpected implementation failure
(`DIVE-P0050`). No input panic is an accepted disposition.

## Evidence

[The source study](../conformance/reports/pluscal-source-grammar.json) accounts
for **222** inputs/markers:

| Outcome | Count | Interpretation |
|---|---:|---|
| Source AST match | 200 | Complete ordered operands/branches, declarations, definitions, macros, procedures and process identity/fairness match secondary grammar ASTs |
| Grammar rejection match | 13 | Both reject; native categories do not claim identical Java error text |
| Reference stage pending | 4 | Java GetMacro expands earlier calls; native preserves source. These are not full AST matches |
| Oracle timeout | 1 | Github358 does not finish within three seconds; native syntax error claims no oracle agreement |
| Selection error | 4 | Documentation markers lack standalone terminators; none is silently dropped |

Clean primary `getAlgorithm` accepts 201 selected inputs, rejects 16 and times
out on one. Three matching source ASTs require missing-label normalization that
this entry rejects: two curated partially labelled loops and upstream LabelTest1.
Their normalization-pending statuses remain in the report. No source result
certifies normalized/translated output or the complete F-034 family.

Selection scans every `.tla` in pinned Examples (`6a7b26a`) and tlaplus
(`94d0c50`), preserving all 159 markers, file hashes and byte regions. Fifty-seven
owned grammar controls and six edge controls supplement the extracts. Inputs,
raw primary/secondary responses, hashes and MIT notices are retained under
`crates/dive-pluscal/acceptance/`. Transcript gzip is lossless; digests identify
decompressed bytes.

`cargo test -p dive-pluscal` compares the complete selection offline without
Java. Ten full-document mutations must fail: wrong names/fairness, removed or
unowned nodes, altered source/tokens, internal errors and false translation
claims. Separate controls cover UTF-8/ranges, display positions, resource limits
and deep input on a normal 2 MiB caller stack. F-034 remains **partial**.

## Reproduction

See [acceptance/README.md](../crates/dive-pluscal/acceptance/README.md). Java/javac
are development probe inputs only. The secondary adapter never replaces the
translation oracle. Generated `conformance/golden/` files are untouched.

Independent read-only review also ran 3,000 deterministic UTF-8 token mutations
across C/P/Unicode source templates, with no internal errors or invalid token,
statement or diagnostic spans. Its seed was `0x59ba47138`; out-of-range and
zero/tiny/maximum-depth caller options were additional controls.
