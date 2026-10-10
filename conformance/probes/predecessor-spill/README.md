# Predecessor spill delivery evidence

ADR 0031 and `docs/predecessor-storage.md` define the separate transient log
scope. The initial development equality control removed a nonexistent `timings`
field but retained the actual `elapsed_ms`; two semantically identical runs
therefore failed strict JSON equality on 4 ms versus 2 ms. The preserved log
`initial-timing-field-failure.txt` contains that failure. The corrected control
excludes only elapsed time and the requested storage options/evidence; all
verdict/count/trace/output/worker fields remain strictly equal. No implementation
or oracle result was changed to obtain that pass.

The fixed source passed 713/713 workspace tests, 20 Python tests, Clippy,
fmt, doc tests, rustdoc with denied warnings, layering, standalone trace and deny.
`delivery.json` records release binary/harness/wrapper identities. Required CM1
21/21 and ValidationTestSuite 16/16 agree at 1/4/8 workers. Both selections also
fully agree through a wrapper that forces zero predecessor capacity. Wrapper
bytes are machine-specific; its portable execution template and exact tested
hash are recorded separately from the native binary hash.

`raw/index.json` retains all 936 native stdout/stderr runs: 13 existing captured
specs, both profiles/stores, widths 1/4/8, normal/chaos, and memory/0/24-byte log
settings. Every bounded run strictly equals its memory reference except elapsed
time and the requested storage options/evidence. This is functional evidence,
not a throughput claim or family certification. Reproduce with:

```sh
python3 conformance/probes/predecessor-spill/capture_controls.py \
  target/release/dive <output-directory>
```

The interrupted first gate is preserved. Self-review replaced brittle diagnostic
substring matching with a typed bounded first-failure slot, then reran the entire
fixed-source gate. Caller text containing DIVE-S0300 cannot become storage evidence;
a later closed-log/cleanup error cannot replace the original write operation.
No oracle capture or golden was altered.

Seen sets, candidate vectors, graph/SCC state, decoded/reconstructed values and
allocator workspaces remain outside this record-capacity budget. No checkpoint,
restart or total-memory claim is made.

## Combined frontier integration

The original delivery and its 936 raw results are retained byte for byte; its
source hashes refer to gzip copies in `historical-delivery-source/`. The current
API carries independent frontier and predecessor budgets. Integration controls
force both to zero on complete and stopping searches, both value profiles,
both identity stores and 1/4/8 workers, and independently replay violations.
Only elapsed time and the explicitly requested storage options/accounting are
excluded from semantic comparison; verdicts, counts, errors, output, worker
evidence and concrete traces are compared. Neither budget bounds total memory.
