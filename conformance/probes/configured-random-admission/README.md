# Configured native RNG admission (#180)

Ordinary safety checking refuses `.cfg` native random replacements before
binding, startup output or search, in both profiles. The pre-effect scan matches
RHS names through module contexts and source-native override authority. It covers
`RandomElement`, `RandomSubset`, `RandomSetOfSubsets` and the typed native
`RandomSubsetSet` helper, including source-visible/configured mixed calls and
transitive constant substitutions. Matching all contexts is conservative: unused,
shadowed or malformed native target matches can refuse before a binding error.
No flag enables the unproved route. Pure user replacements and direct source RNG
calls without native configured targets retain their supported implementations.

This is a native soundness repair, not a TLC divergence or a stream port. The
actual bound target must eventually drive both inline scheduling and per-state
RNG selection, with independent concrete trace replay. Recovery frames must use
that same authority. No seeded recovery, transformed-fingerprint RNG or parity
certification claim is made here.

## Before controls

The frozen [primary/native source and outputs](../view-random-seeding/no-view-configured-follow-up/index.json)
have identical four-state counts but different concrete draws at seed 7/fp 0:
TLC `0,80,94,100`; native `0,74,75,35`. The bounded four-state model makes the
soundness consequence explicit:

```tla
EXTENDS Naturals, TLC
CONSTANT C(_)
VARIABLES n,x
Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=C(1..100)
Inv == x # 80
```

Use `CONSTANT C <- RandomElement`, `INIT Init`, `NEXT Next`, `INVARIANT Inv`
and `CHECK_DEADLOCK FALSE`. The [missed-violation control](../view-random-seeding/no-view-configured-follow-up/missed-violation/index.json)
has TLC violation at x80 but native false success. Setting `Inv == x # 74`
produces [the false-violation control](../view-random-seeding/no-view-configured-follow-up/false-violation/index.json):
TLC succeeds; native reports a violation at x74. Both immutable old and newer
root executable observations retain separate exact source/binary identities.

The [baseline public checker regression](before/regression.log) ran against
production parent `a3b1ee365335e64c04f9109fe839443da6b0dc8d` and failed the strict
Unsupported/no-work assertion before the repair. Its exact test source is retained.
The first expanded test attempted to use `RandomSubsetSet` as an exported source
name; binding correctly reported undefined identifier 2224. That failed source/log
is retained under `development/undefined-native-helper/`. Its native-authority
control now uses admitted public typed IR, without changing implementation,
oracles, parser behavior or the strict refusal assertion.

## Current controls and scope

Checker negatives: 288 settings across both profiles, both stores, widths 1/4/8
and chaos off/19. They include both wrong-verdict models, startup/Init effects,
mixed visible/configured calls, natural/instantiated/typed targets, all native
families and a transitive constant target. Every negative requires exact named
Unsupported, requested width retained but used zero, and no counts, collision,
output, warnings or counterexample. Pure configured replacement has 24 positive
settings; existing direct RNG/pure VIEW and pre-bind VIEW/RNG controls stay intact.
Public file/API negatives cover another 48 settings with startup effects.

Source authority refresh is separate from these runtime observations. Original
receipts and producer/source maps stay immutable; the before-authorities archive
preserves their exact bytes before current integrity maps are recomputed. Fresh
full-gate, executable and conformance receipts will be recorded separately.

## Capture and alternate backend audit

The legacy supplied-`Model` capture path also missed configured native RNG.
Its [exact Rust consumer/source/executable/rlib authority](backend-before/capture-index.json)
returned a false TLC-profile invariant violation; pooled capture already refused
on context purity. Every capture route now checks admitted constant bindings
and actual native definition redirects, without callbacks, before its host/search.
Pure legacy captures with unused TLC native imports and pure replacements remain
supported. Capture callers have already constructed/bound their supplied model:
this guard does not undo their earlier startup effects or claim pre-binding work.
The controls add 192 capture negatives across profiles/stores/1/4/8/chaos and
both inline/pool endpoints, plus pure legacy positives.

Property-bearing public final routes add 192 no-work/refusal settings, preserving
both wrong-invariant models and two liveness predicates. The frozen clean
[primary/property-bearing audit](backend-before/property-audit-index.json)
with `Live == []<>(n>=0)` succeeds, but the old compat final/short path reported
false invariant violation x74. Pinned final/TLC selection already refused the
impure configured target before binding; default pinned selection refuses its
profile. The initial `Live == <>(n=3)` control has a genuine TLC temporal violation
because fairness is absent. Its original failed exit-zero assumption and raw
source remain separate; no primary bytes were changed to obtain success.

The recorded alternate CLI observations at frozen producer4ec show simulation
and DFID refusing unresolved constant-operator effects, and checkpoint refusing
runtime effects in RandomElement. These backends do not pass through the ordinary
guard; their observed refusals are not evidence of a proved configured stream.
The [source/commands/raw observations](backend-before/index.json) retain their
actual stage and output. No cross-backend seeded stream claim is made.

The first full gate after current-main integration stopped on two Clippy
`assert_is_empty` style checks in the no-output malformed-context controls. The
[exact source/log](development/current-main-clippy-before/index.json) is preserved;
assertions now use strict empty-vector equality. Production and oracle bytes were
unchanged. The current-main source authorities are archived independently under
`current-main-before-authorities/` before refreshing current integrity hashes.
