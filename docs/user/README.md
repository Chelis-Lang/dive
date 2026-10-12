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
| Translate or edit algorithms | [PlusCal](pluscal.md) | Admitted C/P translation, bounded source-option context and in-memory Full/body-only editing; process/procedure translation has a wider scope than editing |
| Validate implementation traces | [Trace validation](traces.md) | Complete states, independent predicate replay |
| Inspect public Strict contexts | [Strict context admission](strict-context-admission.md) | Pre-entry refusal; ordinary evaluator semantics unchanged; receiving qualification pending |
| Replay tuple sequence calls in TLC mode | [Tuple-only replay](../tuple-only-trace-replay.md) | Unevaluated shape proof; variable stack histories remain refused |
| Replay tuple-valued variables | [Inductive tuple replay](../inductive-tuple-trace-replay.md) | Bounded Init/all-branches Next proof; arbitrary states and general saved stacks remain refused |
| Replay finite process-owned tuple stacks | [Indexed stack replay](../indexed-stack-trace-replay.md) | Bounded Init/all-Next function-range proof; arbitrary-state and general histories remain refused |
| Check temporal properties | [Temporal checking](temporal.md) | Explicit final/short subset; ordinary strict path refuses |
| Build a periodic backend | [Periodic controller](../ordinary-periodic-controller.md) | Callback ordering only; ordinary temporal verdicts remain unavailable |
| Manage search storage or resume | [Storage and recovery](storage-recovery.md) | Separate budgets and bounded native recovery |
| Use private disk membership in Rust | [Disk seen-state library](disk-seen-library.md) | Fallible additive library; the separate opt-in safety route is available |
| Embed dive in Rust | [Rust API](rust-api.md) | Per-operation reports, errors and explicit options |
| Interpret structured results | [Reports and errors](reports-and-errors.md) | Versioned schemas; exit zero alone is insufficient |

These guides describe the implementation in this checkout. Feature studies and
ADRs retain historical measurements separately. Read [current coverage](../sprint-status.md)
and the [maintained TLC differences log](../tlc-differences.md) for known limitations
and approved better defaults. LSP, native compilation and distributed recovery
are not exposed as completed functionality here.

Concrete temporal state representation: [Stored successor values](../stored-successor-material.md).

- [Source-owned periodic backend diagnostic](../periodic-backend.md): pointer growth, explicit partial requests and real native-source stopping; no ordinary clock schedule or model-proof claim.

- [Native clock/dispatch lifecycle diagnostic](../native-periodic-lifecycle.md): real pauses, internal timer observations and EOF checks; ordinary scheduling and model proof remain unsupported.

- [Opt-in disk seen-state safety checking](disk-seen-safety.md): explicit bounded
  retention, inline fallback, typed failure/cleanup facts and executable Rust example.

- [Stopped-worker periodic diagnostics in Rust](../stopped-periodic-service.md):
  original source winner, private transaction facts and separate diagnostic faults.


The following receiving additions have component-level evidence. They still
require a fresh combined build and campaign before shipping qualification:

- [Recursive procedure checksum editing](../pluscal-recursive-procedure-checksums.md)
  describes the admitted sequential recursive forms, Full/body-only editing and
  CLI/Rust examples. General saved-stack replay still refuses.
- [Optional configured periodic hooks](../optional-periodic-hook.md) describes
  the bounded diagnostic route without a synthetic hook.
- [Terminal periodic diagnostics](../terminal-periodic-service.md) and
  [stopped-worker diagnostics](../stopped-periodic-service.md) describe real
  owned EOF and first-error prefixes. These APIs preserve source errors and
  cannot establish ordinary temporal model proof.
- [Pure disk safety properties](disk-seen-safety.md) describes admitted pure
  state/action properties and safety-only SPEC roles, with named pre-execution
  refusals for temporal or effectful roles.

- [Terminal outcomes and final/post composition in Rust](../terminal-outcome.md):
  actual graph completion, independently replayed words and bounded direct posts;
  ordinary scheduling remains unsupported.
