# Using dive

dive checks TLA+ specifications without a JVM. It reads your existing `.tla`
and `.cfg` files and gives TLC's answer on every model it checks in the
tlaplus/Examples corpus, apart from the
[documented differences](../tlc-differences.md). At one worker it takes about a
fifth of TLC's time on the geometric mean, with no model above twice TLC's
time.

One binary covers parsing and analysis, PlusCal translation, safety and
temporal model checking, simulation, iterative deepening and independent trace
validation. When dive meets something it does not implement, it refuses by
name. A refusal or a run stopped by a budget proves nothing about
the model.

| Task | Guide | Scope |
| --- | --- | --- |
| Install dive | [Install](install.md) | crates.io, Linux archives, source |
| Check a first model | [Getting started](getting-started.md) | Counter example, written inline |
| Check safety properties | [Safety checking](checking.md) | Exhaustive BFS, safety symmetry |
| Check a large finite state space | [Finite symbolic safety](symbolic.md) | Decision diagrams, edited invariants and induction from a source checkout |
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

`dive --help` and `dive <command> --help` list the commands and options dive
implements. Options it always refuses are left out of the help but still parse,
so an existing script gets a named refusal (verdict `unsupported`, exit 3, the
feature named) instead of a usage error:

| Command | Unlisted option | Refusal names |
| --- | --- | --- |
| `check` | `--lncheck final --lasso short` | bounded final checking (with temporal properties) |
| `check` | `--action-composition tlc`, `-Dtlc2.tool.impl.Tool.cdot true` | `--action-composition=tlc` |
| `check` | `--allow-io=env,files,exec` | external inputs |
| `check` | `--source-coverage`, `--coverage 0` | source coverage and TLC logical coverage |
| `dfid` | `--workers N` with N > 1 | DFID multiple workers |
| `tlc` | `-coverage 0`, `-lncheck final` | coverage; bounded final checking |

Without temporal properties, `--lncheck final --lasso short` changes nothing
and the check runs, and `dfid` accepts `--workers 1`.
`--compat=tlc` and `dive tlc` check temporal properties once at exhaustion
without these flags. `--symmetry-liveness tlc` is listed because it is the
`--compat=tlc` default; it runs unless the model has both `SYMMETRY` and
temporal properties, and that combination refuses by name. There is no
language server: `dive lsp` refuses by name.

The [differences from TLC](../tlc-differences.md) list the known limitations
and every place where dive's default answer differs from TLC's. The
[scoreboard](../scoreboard.md) has current coverage numbers.
