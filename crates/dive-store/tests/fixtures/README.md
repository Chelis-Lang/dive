# `dive-store` TLC fixtures

Captured from the pinned oracle `tlc-2026.10.06.014338` (tlaplus `94d0c50`) on the
pinned Temurin 17.0.20.1+1 runtime. Local paths in the capture commands are written
`$DIVE_CACHE_DIR`, `$CWD` (the Examples checkout `6a7b26a`), `$TMP` and `$OUT`.

## `tlc-fingerprints/`

TLC's fingerprints of every distinct state of two Examples models whose golden prints
the "based on the actual fingerprints" estimate, for the minimum-gap tests in
`src/collision.rs`:

| Model | Distinct states | TLC's estimates |
|---|---|---|
| `specifications/byzpaxos/VoteProof.cfg` | 6,962 | `1.2E-10`, `8.6E-12` |
| `specifications/nbacg_guer01/nbacg_guer01.cfg` | 24,922 | `1.8E-10`, `1.5E-11` |

They were captured by running TLC exactly as the harness does for the golden (the
command recorded in the golden's provenance) with `-dump dot $OUT/<model>` added, and
taking the node ids of the dot file, which are `TLCState.fingerPrint()`. Each
`<model>.fp64` file holds the distinct fingerprints as little-endian 64-bit words,
sorted as unsigned integers. The capture runs' counts and estimates equal the
goldens'.

`nbacg_guer01` is the discriminating case. Its closest pair of fingerprints when the
top bit is ignored is one negative and one positive fingerprint, 5,797,823,074 apart,
which would give `1.7E-10`. TLC prints `1.5E-11` because its default fingerprint set
(`MultiFPSet` with `fpBits = 1`) keeps negative and positive fingerprints in separate
sub-sets and computes each sub-set's minimum gap separately.

## `tlc-var-order/`

TLC's state graphs of the 29 variable-order specifications in
`crates/dive-ir/tests/fixtures/var-order/specs/` (4 states each), for
`tests/tlc_state_fingerprints.rs`. Each `<Name>.dot` was produced in the specification's
directory by

```sh
java -XX:+UseParallelGC -Xmx3g -Xss8m -Duser.language=en -Duser.country=US -Dfile.encoding=UTF-8 \
  -cp tla2tools.jar tlc2.TLC <Name>.tla -config <Name>.cfg -workers 1 -cleanup -tool \
  -fp 0 -seed 0 -metadir $TMP/states -dump dot $OUT/<Name>
```

Every run ended with exit code 0 and 4 distinct states. The node ids are
`TLCState.fingerPrint()` and the labels are `TLCStateMut.toString()`, which lists the
variables in TLC's `varLoc` order.
