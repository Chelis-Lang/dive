# Using dive

dive provides parsing, semantic analysis, native PlusCal translation, explicit
safety checking, finite simulation, DFID, independent trace validation and
temporal checking. Core is the only engine. On the tested tlaplus/Examples
corpus it gives TLC's answer on every model it checks, apart from
[registered differences](../tlc-differences.md), and at one worker it takes about a
fifth of TLC's time on the geometric mean, with no model above twice TLC's
time. What dive does not implement is a named refusal, and a named refusal or
resource stop does not establish safety.

| Task | Guide | Current boundary |
| --- | --- | --- |
| Build and run a first model | [Getting started](getting-started.md) | Native binary; source checkout |
| Parse, analyze and resolve modules | [Front end](front-end.md) | Source/config acceptance is not model checking |
| Use modules and library operators | [Modules](modules.md) | Resolution and executable support are separate |
| Explore safety properties | [Checking](checking.md) | Supported explicit BFS and pure safety symmetry |
| Invoke TLC checking arguments | [TLC checking entrypoints](checking.md#tlc-checking-entrypoints) | Checking-only adapter; tool messages and other TLC mode protocols refuse |
| Match TLC behavior | [Compatibility](compatibility.md) | Proven supported counterparts; explicit mixed modes |
| Sample executions | [Simulation](simulation.md) | Finite seeded sampling; never exhaustive proof |
| Use iterative deepening | [DFID](dfid.md) | Bounded pure safety; inspect completion certificate |
| Coverage (`--coverage`, `--source-coverage`) | [Checking](checking.md#coverage) | Not implemented on Core: a named refusal |
| Debug compiled safety checking | [Debugging](debugging.md) | One actual local Core worker; source/exception stops, forward steps and bounded captured-scalar inspection |
| Translate or edit algorithms | [PlusCal](pluscal.md) | Admitted C/P translation, checksummed in-memory editing or explicit module/cfg/backup publication; separate original-AST file operation |
| Validate implementation traces | [Trace validation](traces.md) | Complete states, independent predicate replay |
| Check temporal properties | [Temporal checking](temporal.md) | Bounded pure subset; ordinary dive profile uses deterministic level checks, TLC profile uses final checking; explicit final/short in both |
| Manage search storage or resume | [Storage and recovery](storage-recovery.md) | Separate spill budgets, opt-in Core value reclamation and bounded native recovery |
| Use trusted native safety workers | [Distributed safety](distributed-safety.md) | Explicit authenticated pure Core safety with ordered commit/retry; no durable coordinator or temporal recovery |
| Use private disk membership in Rust | [Disk seen-state library](disk-seen-library.md) | Fallible additive library; the separate opt-in safety route is available |
| Embed dive in Rust | [Rust API](rust-api.md) | Per-operation reports, errors and explicit options |
| Interpret structured results | [Reports and errors](reports-and-errors.md) | Versioned schemas; exit zero alone is insufficient |

## Command-line surface

`dive --help` and `dive <command> --help` list only the commands and options
that Core implements; `crates/dive-cli/tests/golden/command-help.txt` pins
them. Options that Core always refuses are not listed. They still parse, so
existing scripts get the same named refusal (verdict `unsupported`, exit 3,
the feature named) rather than a usage error:

| Command | Unlisted option | Refusal names |
| --- | --- | --- |
| `check` | `--lncheck final --lasso short` | bounded final checking (with temporal properties) |
| `check` | `--action-composition tlc`, `-Dtlc2.tool.impl.Tool.cdot true` | `--action-composition=tlc` |
| `check` | `--allow-io=env,files,exec` | external inputs |
| `check` | `--source-coverage`, `--coverage 0` | source coverage and TLC logical coverage |
| `simulate` | `--workers N` with N > 1 | parallel simulation workers |
| `dfid` | `--workers N` with N > 1 | DFID multiple workers |
| `tlc` | `-coverage 0`, `-lncheck final` | coverage; bounded final checking |

Without temporal properties, `--lncheck final --lasso short` changes
nothing and the check runs; `--workers 1` is accepted by `simulate` and
`dfid`. `--compat=tlc` and `dive tlc` check temporal properties once at
exhaustion without the explicit flags. `--symmetry-liveness tlc` stays
listed: it is the `--compat=tlc` default and runs unless `SYMMETRY` meets
temporal properties, which it refuses by name. `lsp` is not a listed
command and refuses by name.

These guides describe the implementation in this checkout. Read the
[scoreboard](../scoreboard.md) for current coverage and the
[maintained TLC differences log](../tlc-differences.md) for known limitations
and approved better defaults. LSP, native compilation and distributed recovery
are not exposed as completed functionality here.

Strict TLC-mode trace replay also admits bounded tuple-valued variables, tuple
sequence calls and finite process-owned tuple stacks; arbitrary states and general
saved stacks still refuse. See [trace validation](traces.md).
