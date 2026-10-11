# Checked stored trace transport

This is the opt-in version-2 transport study for #190 and ADR 0055. Version 1
remains the default. Transport construction is separate from independent
Init/Next/ending validation; no representation flag is a proof.

The three `specs/` fixtures cover a forward mixed record/tuple set and nonliteral
functions whose domain is a tuple or a record. `primary/` retains clean pinned
TLC runs (seed 7, polynomial 0, one worker, no dumpTrace): invariant exit 12,
4 generated/distinct states and depth 4. These rendered states/counts do not
establish stored-representation or native-certificate authority.

The API tests retain both profiles/stores, widths 1/4/8 and ordinary/chaos19
scheduling: 72 version-2 originals, 216 altered Init/middle/final states and 72
matching legacy version-1 controls. The value tests separately assert exact
literal/function flags, stored JSON and existing FP64/encoded bytes, bounded
witness/decimal admission and refusal without additional provider calls. Current
release/gate receipts will separately qualify the producing executable; focused
test coverage alone is not a full-gate receipt.

`before-current-source-authority/` preserves original maintained-map bytes and
changed parent Rust source from the frozen 5398 parent. Refreshed current maps
are integrity authority, never a retag of old runtime observations. The complete
legacy #190 132-original/396-mutant evidence remains untouched, with its shape
failures excluded from semantic credit.

`codec-tuple-loss/` is a separate #195 study on the immutable d749 producer.
The unsuccessful first comparison probe is retained. The application probe
shows clean TLC and memory-native function-domain diagnostics, while zero-byte
frontier spill and checkpoint recovery report a tuple-index diagnostic because
the storage codec drops the tuple-function literal bit. Predecessor-only spilling
preserves the diagnostic in this minimal control. Storage formats are unchanged
by version 2; no Java byte interoperability or archive certificate is claimed.

`before-style-gate/` preserves the first frozen 249f gate: 1,156 tests pass with
zero skipped, and all other stages pass except a workspace Clippy helper
placement diagnostic. It contains 443 current workspace Rust/Cargo inputs,
excluding unrelated plaintext historical acceptance snapshots; the original
804-input superset index/source archive is retained separately with the original
producer. A later encoder-shape hardening recomputes ordinary function and Raw
constructor shape before emitting v2, refusing trusted noncanonical tags/flags.
Neither the failed gate nor its source is retagged as the corrected producer.

`before-decoder-admission-gate/` separately preserves the green 92e full gate
(1,157 passed, zero skipped, all eight stages) and the later compiled nested-Raw
counterexample: encoder success followed by same-context decoder refusal in both
profiles. A corrected encoder uses owned reconstruction plus exact shape before
emission. Outer Model-based writer controls also reject unknown or mismatched
Atom tokens without interning; neither earlier execution is retagged as the fix.
