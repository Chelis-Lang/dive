# Native DFID review, 2026-10-10

Scope: the pure single-worker safety slice in ADR 0021. This review does not
certify the complete F-025 acceptance matrix, fresh worker RNG distribution,
raw TLC output or DFID collision-probability reporting.

The clean primary selection contains 159 runs of the unmodified pinned TLC:
87 agree on the documented deterministic fields, 42 retain explicit worker-RNG
ambiguity, 24 receive a named native refusal, and six binding-rejection cases
remain unverified for exact error class. There are no disagreements within
those comparison claims. The separately labelled injected-worker reference
has 168 controls, checked in both stores (336 native runs). All 697 primary
and secondary fixture hashes verify. The raw primary and secondary evidence
is retained under `crates/dive-check/tests/fixtures/dfid/`.

An independent reviewer used a separate checkout, target and cache and found
an implementation blocker: a public caller could inject an effectful captured operator into
a statically pure invariant, causing `PrintT` to run before the engine claimed
completion. The final preflight now follows context and redirect bindings
without forcing them and refuses opaque operator/function/set captures. The
regression also checks a caller-replaced enumerated action, unknown expression
ids and lazy captured unknown ids. Nested captured functions/sets refuse before
effects. Live checker-state reads, including `TLCGet("level")`, also refuse
before initialization.

The reviewer also demonstrated an evidence blocker: the first comparison
script excluded every check on worker-ambiguous rows, allowing fake completed,
uncertified-violation and internal-error documents to pass as ambiguity. The
repaired script excludes only unstable counts/trace choice. Stable deciding
class/code, exit, completion message, independent validation and proof scope
remain mandatory. Negative mutations preserve this distinction in the gate;
ambiguity never becomes primary agreement. A further argument check now
matches TLC's zero-worker code 1102 and exit 1 before model loading, rather
than incorrectly applying DFID's multi-worker error 1000.
The reviewer independently retested both fixes: the nonexistent-model/zero
width request returns 1102/exit 1 before loading, and all three full-schema
comparison forgeries now return disagreement and a nonzero comparator exit.

After that fix, the independent controls passed:

- 4,800 randomized finite directed-graph checks compared completion,
  reachability and concrete violation paths with a separate graph traversal.
- 3,024 resource-cut combinations never proved a proper explored prefix.
- 24 held witnesses independently validated; 72 initial/Next/ending mutants
  were rejected or explicitly undecidable.
- Missing inputs, invalid worker widths and invalid fingerprints preserved
  unknown or partial execution evidence.
- The opaque operator, nested closure and invalid captured-expression
  controls produced named refusals without effects or panics.

The delivery's own tests additionally cover 2,000 retained/generated/witness
budget combinations and exact seeded visitation/count fields in both stores.
Initial/action properties combined with relevant constraints remain named
refusals until a separate independent witness contract covers their deciding
order. D-019's stale TLC root witness is preserved as raw reference evidence;
native claims require the actual concrete path to validate independently.

Original delivery validation: the full local gate passed, including 607 workspace tests,
doc tests, documentation with warnings denied, both Python gate suites,
Clippy, dependency/layer checks and the standalone independent validator build.
No golden, expected oracle outcome or acceptance criterion was weakened.

Integration follow-up: the clean primary controls were rerun after the runtime
namespace repair (#122/#124) and shared eager configuration setup. Six previous
agreements now explicitly refuse independent validation because encounter
history cannot be established from a supplied witness. The earlier report is
retained as `conformance/reports/dfid-primary-controls-pre-namespace-repair.json`;
its 93/42/18/6 totals describe that earlier binary only. Inline name-only
execution remains supported in the core, while live effects stay refused.
Comparator metadata checks now apply to every disposition, including refusals
and binding failures, so neither can hide a false proof or a forged exit code.
