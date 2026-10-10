# Temporal conditional identity repair (#170)

This increment repairs bounded TLC normalization and tableau projections. It
**does not yet enable temporal-kind IF in the final checking API or CLI**: the
existing named admission refusal remains until the next execution/witness slice.
There is no flag/default or ordinary periodic-schedule change.

```tla
Live == [](IF x=0 THEN x=0 ELSE x=1 => <>(x=2))
```

On the pinned build, the IF guard is one shared liveness object. The tag walk
visits it twice and assigns its final tag, 3, to both references. Tag 1 becomes
an unused slot. The earlier Rust importer instead allocated tags 1 and 3 to two
independent objects. This changed initial tableau particle numbering (3 versus
4), even when truth on tested words matched. The original source, failed
assertion and raw captures remain in the [probe directory](../conformance/probes/tlc-temporal-if/).

The corrected importer preserves input arena-node identity, updates earlier
references with the last assigned tag, and preserves every visit/unused slot.
Separately constructed predicates with equal truth or equal expression text
still have distinct identities. Every rewrite consumes the existing work
budget; map entries and reference arrays are bounded by charged import terms,
not by a claimed aggregate memory/byte bound. No limits were increased.

All 25 selected reference invocations remain recorded: 20 complete diagnostic
normalization/group/tableau projections and five failures. Selected nested,
INSTANCE-body, multiple-property, constant and distinct-object cases retain
exact bins, particle identities and ordered edges. 5,076 independent periodic
word/closing-position controls across 18 pure models preserve truth; this is
language evidence, not a model certificate.

Two temporal-formal forwarding cases (`TiLet`, `TiForward`) are still refused
at final admission: pinned TLC rejects them with 2213 while diagnostic Rust
lowering expands them. That diagnostic gap receives no field/error agreement
credit. Three clean reference cases (`TiThen`, `TiNested`, `TiMulti`) stop early
with message 2116 and exit 12, so a completed helper graph cannot certify their
actual stopping/error contract. All 25 clean outcomes and the separate 15
completed/10 failed final-projection controls are retained without promotion.

Use the [final API guide](tlc-final-api.md) and [pure state-IF guide](tlc-state-if.md)
for currently exposed behavior. Exact temporal-IF execution, independently
validated witnesses and original source-prefix exits remain the next #170 slice.
Full temporal families and ordinary periodic schedules remain partial.
