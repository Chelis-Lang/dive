# Independent enabledness oracle control

`EnabledFiniteCover` checks ENABLED against a known finite arithmetic range at
every assignment in `(-1)..4` squared. Pinned TLC succeeds: 72 generated,
36 distinct, depth 1, exit 0. The independent procedure checks all 36 cases.

From the repository root:

```sh
cargo run -p dive-conformance --release -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-trace/tests/fixtures/specs \
  --out crates/dive-trace/tests/fixtures/tlc \
  --lncheck final --workers 1 --fp 0 EnabledFiniteCover
```

The harness captured raw commands, streams, exits and SANY output. Reason:
prove independent finite-cover enabledness against the pinned canonical tool.
The trace-capture side pass is never used as the deciding result.
