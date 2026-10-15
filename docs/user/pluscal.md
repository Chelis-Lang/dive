# Native PlusCal translation

```sh
dive translate docs/user/examples/Algorithm.tla --output json
```

Translation reads a C- or P-syntax algorithm in a module comment and returns an
in-memory artifact. The sample is a sequential P-syntax loop. Public CLI/API
acceptance requires the generated enclosing module to pass native syntax,
name and level analysis. `accepted: true` means an admitted artifact. Schema2
reports the admission without model-proof or exploration fields.

The JSON includes `module_source`, generated TLA/configuration text, actions,
process metadata, inserted labels, mappings, warnings and stage diagnostics.
Existing translation blocks are replaced in the returned module at their original
position. The command does not write the source file. To save an admitted module,
inspect the result and explicitly write its `module_source`; then check it as a separate
model with an appropriate `.cfg`. Generated files are not a model proof.

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
old body text still needs `--replace-modified`. JSON schema2 records actual
acceptance, analysis and options without proof fields; translation is not model
checking.

Rust callers use `TranslationRequest`, `translate_file` or `translate_source`;
`TranslationReport` provides `accepted()`, `exit_code()` and `to_json()`.

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
effective context. `-noold`, `-nocfg` and `-unixEOL` report artifact preferences;
`filesystem_plan.writes_performed` remains false and configuration requirements
are retained. `dive translate --ast` returns a bounded original-stage AST artifact
in memory, including modern procedure/macro ownership and compound/grouped-label
locations when `-debug` is requested. Availability remains separate from generated
module acceptance and model proof. Measured labelled-compound and flat procedural
procedure-free debug headers can supply exact CRCs. Procedural CRCs are unavailable;
identifier-renaming and simultaneous-ordering ownership, actual `-writeAST` file operations
and specification-backed translators remain named unsupported. See the
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
