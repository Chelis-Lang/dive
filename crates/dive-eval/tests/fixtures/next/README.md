# Successor-generator rule fixtures

Minimal specs that pin one rule of TLC's successor generator each (`Tool.getNextStates`,
`getInitStates`, `processUnchanged`, `enabled`, and the `elements()` order of `x' \in S`).
Each module's header comment states what it pins. `tests/next_probes.rs` runs them
through `dive-eval` and compares verdict, states generated, distinct states, queue,
depth and counterexample length with TLC's transcript.

- `specs/<Name>/<Name>.tla` and `.cfg` are the specs.
- `tlc-2026.10.06.014338/` holds TLC's transcripts, captured as golden generation runs
  TLC:

  ```sh
  dive-conformance dev capture --oracle tlc-2026.10.06.014338 \
      --specs crates/dive-eval/tests/fixtures/next/specs \
      --out crates/dive-eval/tests/fixtures/next <Name>...
  ```

  They are generated, never edited.
- `*.nowarning.stdout` are runs of the same jar with the harness's flags plus `-nowarning`
  (the exact command is in each `.nowarning.command`), which `dev capture` cannot pass.
  Paths are replaced by `$CWD`, `$TMP` and `$DIVE_CACHE_DIR` as in the harness's
  transcripts. `NxFastPath` generates 37 states there instead of 7.

| Fixture | TLC (generated / distinct) | What it pins |
|---|---|---|
| `NxForallOrder` | 3 / 3, `Inv` violated | `\A` in an action: the first binding, then the others last first |
| `NxFastPath` | 7 / 4 | Once every variable is assigned, the rest of the action is evaluated, not enumerated |
| `NxUnchangedChanged` | 3 / 3 | `UNCHANGED` of a variable already assigned another value generates nothing (warning 2143) |
| `NxStepAngle` | 10 / 4 | `<A>_v` drops unchanged successors; `[A]_v` adds the stuttering one |
| `NxInLazyCup` | 10 / 3 | `x' \in S \cup T` of lazy sets lists duplicates; dive refuses it |
| `NxInLazyFilter` | 3 / 3, `Inv` violated | `x' \in {f \in [D -> R] : P}` in `[D -> R]`'s order |
| `NxInRcdOrder` | 3 / 3, `Inv` violated | `x' \in [a : S, b : T]`, the last field fastest |
| `NxLazyArgVar` | 5 / 4 | An argument naming a variable assigns it, in `Init` and `Next` |
| `NxIfCase` | 8 / 7 | `IF` and `CASE` choose the branch to enumerate |
| `NxLetAction` | 5 / 3 | A `LET` action used twice is enumerated twice |
| `NxEnabled` | 4 / 4, `Inv` violated | `ENABLED` through TLC's `enabled` search |
| `NxFcnAction` | 81 / 32 | A lazy function applied as an action (`getFcnContext`) |
| `NxSubseteq` | 17 / 4 | `x' \subseteq S` enumerates `SUBSET S` |
| `NxUnassigned` | 2 / 1, error 2109 | A successor with an unassigned variable is generated, then reported |
| `NxInitFastPath` | 4 / 2 | The same shortcut for initial states |
| `NxInitDup` | 5 / 2 | Duplicate initial states are generated |
| `NxInstance` | 4 / 4 | An instance action assigns the variable its variable is substituted by |
| `NxForallEmpty` | 3 / 3 | `\A` over the empty set is its continuation |
