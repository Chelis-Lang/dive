# Using dive

Dive currently provides parsing, semantic analysis, bounded native PlusCal
translation, explicit safety checking, finite simulation, DFID, independent
trace validation and opt-in final/short temporal checking. Full TLC parity is
still in progress. A named refusal or resource stop does not establish safety.

| Task | Guide | Current boundary |
| --- | --- | --- |
| Build and run a first model | [Getting started](getting-started.md) | Native binary; source checkout |
| Parse, analyze and resolve modules | [Front end](front-end.md) | Source/config acceptance is not model checking |
| Use modules and library operators | [Modules](modules.md) | Resolution and executable support are separate |
| Explore safety properties | [Checking](checking.md) | Supported explicit BFS and pure safety symmetry |
| Match TLC behavior | [Compatibility](compatibility.md) | Proven supported counterparts; explicit mixed modes |
| Sample executions | [Simulation](simulation.md) | Finite seeded sampling; never exhaustive proof |
| Use iterative deepening | [DFID](dfid.md) | Bounded pure safety; inspect completion certificate |
| Translate algorithms | [PlusCal](pluscal.md) | Admitted C/P sequential and disjoint finite processes, bounded macros, recursive procedure stacks and process/procedure fairness |
| Validate implementation traces | [Trace validation](traces.md) | Complete states, independent predicate replay |
| Check temporal properties | [Temporal checking](temporal.md) | Explicit final/short subset; ordinary strict path refuses |
| Manage search storage or resume | [Storage and recovery](storage-recovery.md) | Separate budgets and bounded native recovery |
| Embed dive in Rust | [Rust API](rust-api.md) | Per-operation reports, errors and explicit options |
| Interpret structured results | [Reports and errors](reports-and-errors.md) | Versioned schemas; exit zero alone is insufficient |

These guides describe the implementation in this checkout. Feature studies and
ADRs retain historical measurements separately. Read [current coverage](../sprint-status.md)
and the [maintained TLC differences log](../tlc-differences.md) for known limitations
and approved better defaults. LSP, native compilation and distributed recovery
are not exposed as completed functionality here.
