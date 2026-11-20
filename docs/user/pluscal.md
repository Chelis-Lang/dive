# PlusCal translation

```sh
dive translate docs/user/examples/Algorithm.tla --output json
```

`dive translate` reads a C- or P-syntax algorithm from a module comment and
returns the translated module in memory. The sample is a sequential P-syntax
loop. `accepted: true` means the whole generated module passed dive's syntax, name
and level analysis. The schema 4 report has no model-checking or exploration
fields, because translating a module checks nothing about its behavior.

The JSON includes `module_source`, the generated TLA+ and configuration text,
actions, process metadata, inserted labels, mappings, warnings and per-stage
diagnostics. Each mapping links an emitted action definition, by its complete
UTF-8 byte range in `artifact.tla`, to the labeled statement it came from in
the input. Ranges cover multi-line bodies and wrapped updates. When the translation
needs no program counter, the action is named `Next` or after its process
instead of after the label; a mapping still points at the right statement, and
so does one for a renamed label. Mappings link source to output only; they are
not debugger locations.

An existing translation block is replaced in the returned module at its
original position. The default command does not write the source file.

## Writing the result

To write the module, with a `.old` backup and a merged `.cfg`, add `--write`:

```sh
dive translate docs/user/examples/Algorithm.tla --write --output json
dive check docs/user/examples/Algorithm.tla --seed 7
```

`--write` uses checksummed rewriting and protects modified bodies. It writes
only after generation and analysis of the complete module succeed. Before
committing, it checks the source, configuration and backup bytes again, stages
the outputs and replaces the module last. An edit made in the meantime makes it
refuse. Each replacement is atomic, but the group of files is not atomic
across a crash. If the commit fails, dive restores the previous files; if the
restore fails too, the original bytes stay in a staging file and the report
gives its path.

Input, configuration and backup files must be regular UTF-8 files of at most
4 MiB; symlinks and directories refuse.

The `.old` file holds the exact original source. Configuration generation
replaces only the generated prefix before `\* Add statements after this line.`,
keeps your lines, and does not add a second uncommented SPECIFICATION
directive.

## Supported constructs

The [PlusCal translation guide](../pluscal-translation.md) lists the supported
constructs, option flags, limits and refusals. Supported constructs include
sequential code, automatic labels, macros, `define` helpers, symbolic process
domains, scoped renaming, fairness, and procedures, including recursion and
processes that call shared procedures. Resource limits and restrictions on
generated operator names apply in both profiles.

The checked-in process examples translate directly:

```sh
dive translate docs/user/examples/MultipleCallExample.tla --compat=tlc --output json
dive translate docs/user/examples/FairCall.tla --compat=tlc --output json
```

Both return accepted in-memory translations. Read `module_source` before you
use the result.

## Rewriting the translation block in memory

```sh
dive translate docs/user/examples/Algorithm.tla --rewrite --compat=tlc --output json
```

Rewriting always records the body CRC. For supported procedure-free shapes it
also adds TLC's exact algorithm CRC; otherwise it writes
`chksum(pcal) \in STRING` in its place, which turns that check off, and
warns. A missing optional CRC
never blocks generation, rewriting or debug output. `--checksums full|tla` are
aliases for this best-effort behavior. A changed algorithm regenerates
normally. An old body that was edited by hand, or whose checksum cannot be
verified, needs `--replace-modified`.

By default the CLI and API protect a modified or unverified existing body, and
`--replace-modified` overwrites it. `--compat=tlc` overwrites it automatically,
as TLC's translator does. The low-level `rewrite_document` keeps whatever
policy its caller sets. `requested_editing` records the settings you asked for,
`editing` the effective ones, and the old body's CRC and match data stay in the
report.

Recognized `PlusCal options (...)` declarations are read from the original
file, including any existing translation block, before rewriting masks the old
output. If a declaration that changes the translation overlaps old output,
both checksum policies refuse, even with `--replace-modified`, so requested
fairness is never lost.

## Translator options

Other translator options are `-termination`, `-noDoneDisjunct`, `-lineWidth N`
(default 78, minimum 60), supported `-version V` shapes and `-reportLabels`.
Options declared in the source and options given on the CLI or API combine into
one reported effective context. With `--write`, `-noold` and `-nocfg` suppress
the backup and configuration writes; without `--write` they are only recorded.
`-unixEOL` (CLI only) is recorded but changes nothing: generated text uses LF,
default rewriting keeps the source file's newlines, and `--compat=tlc`
rewriting normalizes to LF. TLC's own flag does not change its output either.

`dive translate --ast` returns the AST of the original algorithm in memory,
including which procedure or macro each statement belongs to and compound or
grouped label locations when `-debug` is given. It exits 0 when the AST is
available. Debug headers for labeled compound statements and for flat code
without procedures carry TLC's exact CRCs; procedures get no CRC. Debug data
for renamed identifiers and simultaneous assignments is not produced, and TLC's
`-spec` family of translator backends is not supported.

`-writeAST` writes `AST.tla` in the current directory, leaves the input,
configuration and backup unchanged, and exits 255, as TLC's translator
does. The report says `writes_performed: true` and `accepted: false`; exit 255
here does not mean the write failed. Rust callers use
`write_original_ast_file(input, destination, &TranslationRequest)`. See the
translation guide for limits on historical versions and newlines.

## Rust API

Rust callers use `TranslationRequest`, `translate_file` or `translate_source`;
`TranslationReport` provides `accepted()`, `exit_code()` and `to_json()`. Call
`prepare_translation_file(path, &RewriteRequest)` to inspect a checksummed
rewrite and then `publish()` it, or `write_translation_file` to do both.
`filesystem_plan.writes_performed` and `written_files` report what was written.
