# Parsing, analysis and configurations

```sh
dive parse docs/user/examples/Counter.tla --output json --cst
dive parse --config docs/user/examples/Counter.cfg --output json
dive analyze docs/user/examples/Counter.tla --output json
```

`dive parse` reports syntax, spans and diagnostics; `--cst` adds the lossless
concrete syntax tree. `dive analyze` resolves names, arities, modules and
levels. `dive parse --config` checks a TLC `.cfg` file; whether its names and
values fit the specification is checked later, when a model is bound.
`accepted: true` means the input parsed or analyzed. No invariant or temporal
property was checked.

For analysis, checking, translation and trace validation, repeat
`--library-path DIR` to search a directory before the standard modules, or
`--module-path DIR` to search it after them. When a local module shares a name
with a standard one, the report's file and module list shows which was used.

Rust entry points include `parse_source`, `parse_file`, `parse_config_source`,
`parse_config_file`, `analyze_source` and `analyze_file` in `dive_api`.
Reports retain diagnostics and expose `accepted()` and `to_json(...)`.
Invalid UTF-8 files are decoded with replacement and a `DIVE-S0007` warning;
losslessness then refers to the decoded text.

Diagnostics carry stable codes and source locations, and `dive analyze`
accepts and rejects what SANY does. The syntax and analysis crates can be used
without the model checker.

[Modules](modules.md) explains which library operators dive evaluates natively
and which it reads from their TLA+ sources.
