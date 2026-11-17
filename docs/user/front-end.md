# Parsing, analysis and configurations

```sh
dive parse docs/user/examples/Counter.tla --output json --cst
dive parse --config docs/user/examples/Counter.cfg --output json
dive analyze docs/user/examples/Counter.tla --output json
```

Parsing reports source syntax, spans and diagnostics; `--cst` includes the
lossless concrete syntax tree. Analysis resolves names, arities, modules and
levels. Configuration parsing checks TLC's `.cfg` representation; model binding
later checks how those names and values apply to the spec. `accepted: true` in a
front-end document does not mean an invariant or temporal property was checked.

For analysis, checking, translation and trace validation, repeat
`--library-path DIR` to search before standard modules, or `--module-path DIR`
to search after them. Read the reported file/module list when resolving a
similarly named local and standard module.

Rust entry points include `parse_source`, `parse_file`, `parse_config_source`,
`parse_config_file`, `analyze_source` and `analyze_file` in `dive_api`.
Reports retain diagnostics and expose `accepted()` and `to_json(...)`.
Invalid UTF-8 files are decoded with replacement and a `DIVE-S0007` warning;
losslessness then refers to the decoded text.

Diagnostics carry stable codes and source locations. Source acceptance and
semantic acceptance are independently compared with the pinned SANY, including
on deterministic syntax, layout and semantic mutations. The syntax/analysis crates are
available independently of the model checker.

Read [modules and evaluated operators](modules.md) for the distinction between
resolved module sources and executable native overrides.
