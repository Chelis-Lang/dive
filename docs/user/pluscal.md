# Native PlusCal translation

```sh
dive translate docs/user/examples/Algorithm.tla --output json
```

Translation reads a C- or P-syntax algorithm in a module comment and returns an
in-memory artifact. The sample is a sequential P-syntax loop. Public CLI/API
acceptance requires the generated enclosing module to pass native syntax,
name and level analysis. `accepted: true` means an admitted artifact; it always
retains `model_proved: false` and `exhaustive: false`.

The JSON includes `module_source`, generated TLA/configuration text, actions,
process metadata, inserted labels, mappings, warnings and stage diagnostics.
The command does not rewrite the source. To save an admitted module, inspect the
result and explicitly write its `module_source`; then check it as a separate
model with an appropriate `.cfg`. Generated files are not a model proof.

The supported constructs (sequential code, automatic labels, macros, `define`
helpers, finite processes, fairness and procedures, including recursion and
processes calling shared procedures), the option flags, limits and current
refusals are listed in the [PlusCal translation guide](../pluscal-translation.md).
Resource limits and explicit name and scope restrictions apply in both profiles.

Bounded in-memory translation-block editing is also available:

```sh
dive translate docs/user/examples/Algorithm.tla --rewrite --compat=tlc --output json
```

For admitted shapes, the default Full policy derives the algorithm AST checksum
and the generated-body checksum. An explicit `--checksums tla` request selects
body-only editing and labels TLC mode `tlc+overrides`. The replacement module is
returned in JSON; the command does not write the source file. A matching checksum
alone is not an artifact or model proof. Shapes outside the Full scope refuse by
name; dive never silently falls back to body-only editing.

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
