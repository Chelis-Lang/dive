# Check report schema fixtures

`Reports.tla` and the five configurations exercise seven Core report kinds:
success, invariant violation, deadlock, liveness violation, evaluation error,
named refusal and a state-budget stop. The refusal requests exact storage with
liveness, which is outside the current supported Core storage modes.

`report_schema.rs` runs the CLI, checks the expected exit/verdict/engine and
compares each entire JSON report. It normalizes the fixture directory, elapsed
timings and the operating-system memory metric. Semantic counts, property
names, error codes, witnesses, options and all field names remain pinned.
Memory sampling and time limits are disabled; the budget-stop fixture uses an
explicit state limit.

Generate these CLI wire fixtures with:

```sh
DIVE_UPDATE_REPORT_GOLDENS=1 cargo test -p dive-cli --test report_schema --locked
```

Review the generated diff, then run the same command without the update
variable. These are CLI report fixtures, separate from the oracle-generated
conformance goldens. Changes must follow the report compatibility policy in
`docs/user/reports-and-errors.md`.
