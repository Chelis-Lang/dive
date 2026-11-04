# Using dive

Dive currently provides parsing, semantic analysis, bounded native PlusCal
translation, explicit safety checking, finite simulation, DFID, independent
trace validation and bounded temporal checking with deterministic level checks
or explicit final/short analysis. Full TLC parity is
still in progress. The reference engine remains the default; native capacity
and TLC-relative performance qualification remain open. A named refusal or
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
| Inspect native source work | [Checking](checking.md#native-source-work) | Opt-in Reference producer expression/generator counters at exact source locations; separate from TLC logical costs |
| Translate or edit algorithms | [PlusCal](pluscal.md) | Admitted C/P translation, checksummed in-memory editing or explicit module/cfg/backup publication; separate original-AST file operation |
| Validate implementation traces | [Trace validation](traces.md) | Complete states, independent predicate replay |
| Check temporal properties | [Temporal checking](temporal.md) | Bounded pure subset; ordinary dive profile uses deterministic level checks, TLC profile uses final checking; explicit final/short in both |
| Manage search storage or resume | [Storage and recovery](storage-recovery.md) | Separate spill budgets, opt-in Core value reclamation and bounded native recovery |
| Use trusted native safety workers | [Distributed safety](distributed-safety.md) | Explicit authenticated pure Core safety with ordered commit/retry; no durable coordinator or temporal recovery |
| Use private disk membership in Rust | [Disk seen-state library](disk-seen-library.md) | Fallible additive library; the separate opt-in safety route is available |
| Embed dive in Rust | [Rust API](rust-api.md) | Per-operation reports, errors and explicit options |
| Interpret structured results | [Reports and errors](reports-and-errors.md) | Versioned schemas; exit zero alone is insufficient |

These guides describe the implementation in this checkout. Read the
[scoreboard](../scoreboard.md) for current coverage and the
[maintained TLC differences log](../tlc-differences.md) for known limitations
and approved better defaults. LSP, native compilation and distributed recovery
are not exposed as completed functionality here.

Strict TLC-mode trace replay also admits bounded tuple-valued variables, tuple
sequence calls and finite process-owned tuple stacks; arbitrary states and general
saved stacks still refuse. See [trace validation](traces.md).
