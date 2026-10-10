# Configured-key parallel recovery on current main

`verification.json` identifies frozen source d0d95389f413edb04b38392918f0c98585682724, its executable hashes, 894 passing workspace tests, all eight Rust gates, 20 Python tests and 12 source checks. The 620 retained Rust/build-related inputs are an integrity superset, including historical evidence files; that count is not a compilation count.

Fresh execution contains 144 recovery groups and 1,008 CLI runs with independent concrete witness controls, 176 stopping-limit controls, and 36 record-codec controls. CM1's 21 models and ValidationTestSuite's 16 smoke models pass at requested widths 1, 4 and 8. Captures remain beside their summaries. The comparison executable's original path was retired during owner-requested stale-build cleanup; its identical bytes are retained in the cleanup artifact manifest.

The record helper's initial header hardcoded its earlier producer. `records/inherited-helper-header.json` preserves that emitted header. Only this fresh run's index records the actual frozen source with an explicit correction; no historical capture was relabelled.

The previous 873-, 847-, 831- and 738-test sources and observations remain separate. Recovery admits bounded pure safety VIEW/symmetry and uses the deterministic worker pool. Temporal/runtime effects, transient-spill composition and independent archive certificates are outside this slice. No feature-family certification follows from these controls.
