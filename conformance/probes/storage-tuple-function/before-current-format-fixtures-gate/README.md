# First structural-format-two gate

Immutable producer dc0f97d424a99d669c040062d7fd197eb6d4ebf0 ran all 1167
nextest cases: 1160 passed, seven failed, zero skipped. All other seven gate
stages passed. Existing test payload rewriters in native_checkpoint,
checkpoint_keys and checkpoint_runtime_names emitted the previous CP01 header.
The new production correctly refused that header before reaching their intended
counter, configured-key and name-table checks. A generic corruption control
could also pass solely at version admission. The corrected epoch migrates only
current-payload fixture headers to CP02 and requires that generic corruption
reach payload admission; separately captured old CP01 archive refusals remain.
Full assertions, selections, widths and limits are preserved. No gate or runtime
execution is retagged.
