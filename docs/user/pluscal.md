# Native PlusCal translation

```sh
dive translate docs/user/examples/Algorithm.tla --output json
```

Translation reads a C- or P-syntax algorithm in a module comment and returns an
in-memory artifact. The sample is a sequential P-syntax loop. Public CLI/API
acceptance requires the generated enclosing module to pass native syntax,
name and level analysis. `accepted: true` means an admitted artifact. Schema3
reports the admission without model-proof or exploration fields.

The JSON includes `module_source`, generated TLA/configuration text, actions,
process metadata, inserted labels, mappings, warnings and stage diagnostics.
Each mapping records the emitted action definition's complete UTF-8 byte range
in `artifact.tla` and its original labelled statement owner in the input source.
Ranges include multi-line bodies and wrapped updates. Program-counter omission
can emit `Next` or a process name instead of the original label; scoped label
renaming retains the same original owner. These are artifact source associations;
compiled instruction maps and debugger execution locations remain unfinished.
Existing translation blocks are replaced in the returned module at their original
position. The default command does not write the source file. To publish the admitted
module with a `.old` backup and merged `.cfg`, select `--write`:

```sh
dive translate docs/user/examples/Algorithm.tla --write --output json
dive check docs/user/examples/Algorithm.tla --seed 7
```

`--write` uses checksummed rewriting and the existing modified-body protection.
It writes only after generation and complete-module analysis succeed. It checks
source/configuration/backup bytes again before committing, stages the outputs,
and replaces the module last. Concurrent edits observed before commit refuse;
individual replacements are atomic, but the group is not crash-atomic. An ordinary
commit failure restores prior files; a failed restoration retains original bytes
in a staging file and reports its recovery path. Regular
UTF-8 input/configuration/backup files are bounded to 4 MiB; symlinks and directory
targets refuse. Generated files are not a model proof.

The supported constructs (sequential code, automatic labels, macros, `define`
helpers, symbolic process domains, scoped renaming, fairness and procedures,
including recursion and processes calling shared procedures), the option flags,
limits and current
refusals are listed in the [PlusCal translation guide](../pluscal-translation.md).
Resource limits and generated-operator collision restrictions apply in both profiles.

Bounded in-memory translation-block editing is also available:

```sh
dive translate docs/user/examples/Algorithm.tla --rewrite --compat=tlc --output json
```

Rewriting always records the real body CRC. It adds an exact algorithm CRC for
supported procedure-free shapes; otherwise it disables just that value with the
pinned `chksum(pcal) \in STRING` spelling and reports a warning. Optional CRCs never
block generation, rewriting or debug output. `--checksums full|tla` are aliases for
best effort. Changed algorithm text regenerates normally; edited or unverified
old body text still needs `--replace-modified`. JSON schema3 records actual
acceptance, analysis and options without proof fields; translation is not model
checking.

Rust callers use `TranslationRequest`, `translate_file` or `translate_source`;
`TranslationReport` provides `accepted()`, `exit_code()` and `to_json()`.
Use `prepare_translation_file(path, &RewriteRequest)` to inspect a checksummed
rewrite and call `publish()` explicitly, or `write_translation_file` for the
combined operation. `filesystem_plan.writes_performed` and `written_files`
report actual publication. The `.old` file receives the exact original source;
configuration generation replaces only the old generated prefix before
`\* Add statements after this line.`, retains user lines, and avoids adding a
second uncommented SPECIFICATION directive.

Checked-in process examples can be translated directly:

```sh
dive translate docs/user/examples/MultipleCallExample.tla --compat=tlc --output json
dive translate docs/user/examples/FairCall.tla --compat=tlc --output json
```

Both return admitted in-memory translation artifacts. Inspect `module_source`
and the nonproof fields before using the result.

Recognized `PlusCal options (...)` declarations are inspected in the original
file, including an existing translation block, before rewriting masks old output.
Both checksum policies refuse a deciding declaration that overlaps old output,
even with `--replace-modified`, so requested fairness is never silently lost.

Additional translator options include `-termination`, `-noDoneDisjunct`,
`-lineWidth N` (default 78, minimum 60), supported `-version V` shapes and
`-reportLabels`. Source declarations and CLI/API settings share the reported
effective context. `-noold` and `-nocfg` suppress actual backup/configuration writes with `--write`;
without it they only report preferences. Source options take effect too.
Configuration requirements remain available. `-unixEOL` retains the documented
newline policy; it does not force a new one. `dive translate --ast` returns a bounded original-stage AST artifact
in memory, including modern procedure/macro ownership and compound/grouped-label
locations when `-debug` is requested. Availability remains separate from generated
module acceptance and model proof. Measured labelled-compound and flat procedural
procedure-free debug headers can supply exact CRCs. Procedural CRCs are unavailable;
identifier-renaming and simultaneous-ordering ownership and
specification-backed translators remain unfinished. `-writeAST` writes
`AST.tla` in the current directory, leaves the input/cfg/backup unchanged and
exits 255, matching the pinned operation. The report says `writes_performed:
true`, `accepted: false`; exit 255 here does not mean the AST write failed.
Rust callers use `write_original_ast_file(input, destination, &TranslationRequest)`.
The `--ast` in-memory operation still returns native exit 0 for available bytes. See the
guide for historical-version and newline limits.

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
