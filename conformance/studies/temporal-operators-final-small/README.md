# Complete known-user operator forwarding small census

The unchanged 140-model selection at 600 seconds, one worker and one job ran
against clean `7683204a1ab328ffc2dabb9b932f5c545e129b79`, the frozen known-user
operator producer. All rows are retained: 53 agree, 85 explicit unsupported,
two timeouts, no disagreements. The harness exited 1; this is not an all-green
corpus report. The two timed-out invocations retain null native exits.

`model-by-model.json` compares every selected model to the earlier material
producer (46 agree, 92 unsupported, two timeouts). Seven unsupported rows became
agreements; no old outcome is promoted or relabelled. `before.json` names the
exact source/binary/selection; `source` preserves its git bytes.
`after.json` intentionally excludes the live launcher log. `closed-after.json`
was written after the launcher closed and binds all 559 closed artifacts,
including that log. Raw per-invocation commands, stdout, stderr and dispositions
remain in `raw`; the full harness report remains in `reports`.

This evidence does not describe the later native Len/Head/Cardinality producer,
ordinary periodic liveness, default-profile final support or feature-family
certification. Fixed bounds and busy-host timeout observations stay explicit.

`independent-readback.json` records a separate complete source/artifact/binary
verification of the closed census. It does not replace any original receipt.
