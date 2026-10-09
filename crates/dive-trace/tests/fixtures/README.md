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

`EnabledScopedCover` adds nested scalar/tuple value-bound operators and two
applications of the same relation under different arguments. TLC checks three
ENABLED identities on all 36 initial states (72 generated, depth 1, exit 0).
The independent tests also enumerate concrete successor assignments and verify
every returned witness against the original scoped relation. Capture it with
the command above, substituting `EnabledScopedCover`.

`DeadlockScopedCover` validates the formerly opaque `Step(1)` deadlock: TLC
exits 11 with the concrete 0,1 trace. Capture it with the same command and
its module name. A separate no-finite-cover deadlock remains Unsupported in
the API controls, preserving the absence-proof fallback.


`EnabledQuantifiedCover` is a clean pinned 36-state ENABLED probe. Three
identities cover existential alternatives, inconsistent universal constraints,
and empty existential domains (108 independent decisions). Generated with:

```sh
DIVE_CACHE_DIR=<verified-cache> cargo run -p dive-conformance --release -- dev capture \
  --oracle tlc-2026.10.06.014338 --specs crates/dive-trace/tests/fixtures/specs \
  --out crates/dive-trace/tests/fixtures EnabledQuantifiedCover
```
