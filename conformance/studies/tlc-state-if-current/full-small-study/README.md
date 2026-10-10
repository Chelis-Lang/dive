# Pure state-IF full small census (#168)

The unchanged complete Examples small selection contains 140 models: six agree,
134 refuse by name, zero disagree and zero time out. The harness exited zero.
All 138 real subprocesses and both harness-only refusals remain in the full
report. The real subprocess exit codes are retained; unavailable values are null.

Five agreements use the inherited bounded safety-only route. The sixth,
`LiveHourClock`, actually checks its temporal obligations in explicit final-only
TLC selection (12 distinct states, 24 generated, depth 1, code 2193). This is
one supported final-mode result, not periodic or whole-family certification.
The prior all-140-refusal census and five-agreement safety census remain
unchanged at their historical source.

`before.json` and `after.json` verify immutable source `03f11881b`, all 603
archived build-related inputs and both executable hashes. The CLI is the current
debug binary with debug information disabled; the harness is the unchanged
baseline safety release executable at source `41c7c8b`. Comparer/independent
validator code is unchanged by this slice. No current-source release or
performance claim is made. The 94.3-second census ran while other development
work continued.

`census.json` preserves all invocation outcomes and routing facts. `run.log`,
`reports/` and `raw/` retain the unedited machine reports and streams.
