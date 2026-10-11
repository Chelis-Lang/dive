# Complete stored-successor final-TLC small census

The unchanged 140-model Examples small selection ran at 600 seconds per model,
one worker and one job against production
`c8114616825629d2f54e3d3c7ec224fc1ced91da`. All outcomes remain visible:
60 agreements, 75 explicit unsupported results, five timeouts and no disagreements.
The harness exited 1. This is not a completely passing corpus report.

The full report contains 140 rows. There are 138 original native invocations;
two models receive harness preflight dispositions. Commands, binary hashes,
stdout and stderr remain in `raw/`. The five timed-out reports preserve null
native exit codes, rather than invented success or failure exits:

- `Bakery-Boulangerie/MCBakery`
- `Disruptor/MCDisruptor_MPMC_liveliness`
- `Disruptor/MCDisruptor_SPMC`
- `FlashProtocol/MCFlashWithMutexLive`
- `nbacg_guer01/nbacg_guer01`

`model-by-model.json` compares the entire selection to the closed known-user
operator producer: 53 agreements remain agreements, seven unsupported models
become agreements, 75 remain unsupported, three become timeouts, and two remain
timeouts. Earlier outcomes and receipts are unchanged. This busy-host correctness
census is not a quiet performance benchmark.

`before.json` binds the selection and all 389 build inputs. Its source commit
`29c7a20ff63fe3a8782451862cb0520c8eff19ee` adds packaging metadata to the exact
production bytes. Those inputs are preserved in the
[producer archive](../../probes/stored-successor-material/source-manifest.json).
`after.json` remains the original receipt; the separate `closed-after.json`
binds 556 closed artifacts, including the launcher after it stopped.
`independent-readback.json` records complete Git/blob/gzip, artifact and two-binary
verification. Run `python3 -I scripts/check-stored-successor-census.py` to verify
the repository-contained census and archive again.

These results apply to this bounded explicit final-TLC producer only. They do not
certify ordinary periodic scheduling, default-profile behavior, an entire feature
family, aggregate memory bounds, or a later native-operator/partial-SCC producer.
