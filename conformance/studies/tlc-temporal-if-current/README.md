# Temporal IF shared-tag source receipt

This first #170 slice retains diagnostic language/particle evidence and the
original runtime refusal. All earlier runtime sources remain archived; no
ordinary periodic, final-execution or family certification is claimed. Build
inputs include archived helper/test Rust sources as a conservative superset.

The full local gate passed 876 Rust tests with no skips, 20 Python controls,
and all documentation, source, lint, layering and dependency checks. CM1
reproduced all 21 models at widths 1/4/8 (144 seconds); ValidationTestSuite
smoke reproduced all 16 at those widths (1 second). All 609 archived build
inputs and both exact executables were verified unchanged after these runs.

The current CLI uses the debug profile with debug information disabled. The
conformance executable is the unchanged baseline release identified in the
manifest, whose comparer and independent validator are unchanged. These are
correctness controls, not a fresh release build or performance measurement.
