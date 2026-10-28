# Compound set normalization (#619)

`oracle.json` is the compact result of running every adjacent spec/config with
pinned TLC `tlc-2026.10.06.014338`, upstream
`94d0c5070852b37b801bb8998c436383ab7ee37b`. Its jar SHA-256 is recorded in the
summary. Verdicts/classes/counts are extracted by the existing
`dive_conformance::tlc::result::classify`; tool error-message codes are separate
observations. The pinned Temurin JRE is 17.0.20.1+1. Reproduce outside the repository:

```sh
java -cp "$TLC_JAR" tlc2.TLC -tool -workers 1 -seed 19 -fp 0 \
  -metadir "$RUN_DIR/StreamConstructionClash" \
  -config StreamConstructionClash.cfg StreamConstructionClash
```

`StreamConstructionClash` first builds two comparable tuples at `x=0`; at
`x=1` TLC compares boolean `TRUE` with integer `1` while normalizing for
`Cardinality`, and exits 76 with invariant-evaluation class 2111 and
2 generated / 2 distinct / depth 2. The default core incorrectly reported
success with 3/3/depth 3 on main before this fix. C2 requires normalization
errors in both profiles; D-009's structural equality improvements do not
permit this success.

The reversed tuple, later-field, record and nested-set cases fail after the
initial state and exercise the same normalization boundary. Size and earlier
field ordering can let TLC avoid the
incomparable fields: both short-circuit controls succeed on TLC, but the core
conservatively refuses them until it proves TLC's checked comparison order.
Different record domains, homogeneous compounds and an unevaluated branch
remain successful. The scalar case retains its existing refusal. The direct
equality control preserves the approved D-009 default answer and the TLC-profile
refusal; its oracle error is recorded rather than treated as parity. This
control also exposes a pre-existing reference classification limitation: TLC
reports initial-state context 2102 with deciding fingerprint failure 2147,
while the reference reports 2103 (both exit 75 and 1/1/depth 1). The test checks
the preserved core outcomes and Auto/reference consistency
without claiming reference/TLC class parity for this control
([#620](https://github.com/Chelis-Lang/dive/issues/620)).

`core_normalization.rs` consumes every case and summary, checking both profiles,
1/4 workers, and folding/memo/hash-consing controls. Reference error words are
outside this fix's independent-witness qualification; an Auto fallback that
returns one is not a claim of new trace validation coverage.
