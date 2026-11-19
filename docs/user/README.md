# Using dive

dive checks TLA+ specifications without a JVM. It reads your existing `.tla`
and `.cfg` files and gives TLC's answer, apart from
[registered differences](../tlc-differences.md), on every model of the tested
tlaplus/Examples corpus that it checks. At one worker it takes about a fifth of
TLC's time on the geometric mean, with no model above twice TLC's time.

One binary covers parsing and analysis, PlusCal translation, safety and
temporal model checking, simulation, iterative deepening and independent trace
validation. Core is the only engine. When dive meets something it does not
implement, it refuses by name instead of guessing. A refusal or a run stopped by
a budget proves nothing about the model.

| Task | Guide | Scope |
| --- | --- | --- |
| Install dive | [Install](install.md) | crates.io, Linux archives, source |
| Check a first model | [Getting started](getting-started.md) | Included example |
| Check safety properties | [Safety checking](checking.md) | Exhaustive BFS, safety symmetry |
| Run with TLC's arguments | [TLC checking entrypoints](checking.md#tlc-checking-entrypoints) | Checking only; `-tool` and other TLC protocols refuse |
| Match TLC's behavior | [Compatibility](compatibility.md) | `--compat=tlc` and its gaps |
| Check temporal properties | [Temporal checking](temporal.md) | Bounded pure subset |
| Sample executions | [Simulation](simulation.md) | Seeded and finite; never a proof |
| Use iterative deepening | [DFID](dfid.md) | Pure safety, depth-bounded |
| Validate implementation traces | [Trace validation](traces.md) | Complete states, independent replay |
| Translate or edit PlusCal | [PlusCal](pluscal.md) | C and P syntax |
| Debug a safety check (experimental) | [Debugging](debugging.md) | DAP, one local worker |
| Parse and analyze | [Parsing and analysis](front-end.md) | Acceptance only, no checking |
| Use modules and library operators | [Modules](modules.md) | Standard, TLAPS and CommunityModules |
| Coverage (`--coverage`, `--source-coverage`) | [Checking](checking.md#coverage) | Refused by name |
| Interpret results | [Reports and errors](reports-and-errors.md) | Versioned JSON; read more than the exit code |
| Embed dive in Rust | [Rust API](rust-api.md) | Reports, errors and options per operation |
| Spill to disk or resume a search | [Storage and recovery](storage-recovery.md) | Spill budgets, checkpoints |
| Keep seen states on disk | [Disk seen-state checking](disk-seen-safety.md) | Opt-in, one worker |
| Use disk membership from Rust | [Disk seen-state library](disk-seen-library.md) | Library API |
| Spread safety work across machines | [Distributed safety](distributed-safety.md) | Trusted workers, pure safety |

## Command-line surface

`dive --help` and `dive <command> --help` list only the commands and options
that Core implements; `crates/dive-cli/tests/golden/command-help.txt` pins
them. Options that Core always refuses are left out of the help but still
parse, so an existing script gets a named refusal (verdict `unsupported`, exit
3, the feature named) instead of a usage error:

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

The [scoreboard](../scoreboard.md) has current coverage numbers, and the
[TLC differences log](../tlc-differences.md) lists known limitations and the
places where dive's default improves on TLC. LSP, native compilation and
distributed recovery are not available.

Strict TLC-profile trace replay also accepts bounded tuple-valued variables,
tuple sequence calls and finite process-owned tuple stacks. Arbitrary states
and general saved stacks refuse. See [trace validation](traces.md).
