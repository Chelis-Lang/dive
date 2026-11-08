# Public-context regression evidence

These are native Rust API controls, not generated TLC goldens.

`baseline-constructor.rs.txt` was installed as the `context_before` integration
test in a detached worktree at source `8f451ff` and run with:

```sh
cargo test -p dive-eval --test context_before --locked -- --test-threads=2 --nocapture
```

`baseline-run.txt` retains the complete output, with only machine-local directory
prefixes replaced by `<baseline-worktree>` and `<baseline-target>`. The test
catches the panic so the reproduction command exits zero. The observations are:

- Redirect target `DefId(u32::MAX)` panicked in the IR lookup. The run's name table
  grew from 0 to 1 first, proving admission did not precede literal interning.
- `scope_apply` accepted a lazy `ExprId(u32::MAX)` capture.

The other `original-*.rs.txt` files preserve the earlier endpoint/DFID controls
before constructor admission moved their malformed lazy refusal earlier. Their
valid effectful controls and direct malformed endpoint controls remain in the
live tests. Initial development runs failed on the old construction expectation
and on the lost TLC `ErSubstArity` startup diagnostic; both were corrected at the
admission/diagnostic boundary, without changing oracle fixtures or loosening the
zero-effects assertions.

Current controls live in `dive-eval/tests/contexts.rs`,
`dive-trace/tests/contexts.rs`, `dive-api/tests/temporal_forwarding.rs`,
`dive-eval/tests/bounded_domains.rs` and `dive-api/tests/dfid.rs`.
See ADR 0028 for API migration and the limitation of bare numeric IDs.
