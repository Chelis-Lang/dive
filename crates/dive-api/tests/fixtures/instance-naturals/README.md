# Reference native-argument cache controls (#515)

The original `In6` / `RefSubstDouble` reproduction lives in the registered
`reference-instance-naturals` conformance corpus, with an official generated
TLC golden. It was pinned with a reason before the runtime correction. The
reference previously failed at the initial invariant with undefined variable
2280, deciding 2103 / exit 75, 1 distinct / 1 generated / depth 1. TLC and core
succeed with 1 distinct / 2 generated / depth 1.

The adjacent `oracle.json` contains 18 additional consumed controls generated
through the official harness in an external Examples-layout staging corpus.
It records every source SHA-256, the oracle/JRE/classpath/library pins, harness
source/binary identity, generation reason and staged corpus identity. Classified
verdicts/codes/counts come from the generated goldens. Underlying invariant
causes and output come separately from the retained clean tool streams, not the
trace pass. Raw streams, failed runs and earlier source selections remain
external; no golden was edited.

Reproduce by staging each named config with its module (both StartupStored
configs use `StartupStored.tla`), plus the three helper modules, into its own
Examples-format manifest/model directory. Use the repository's pinned
`oracles.toml`, an external `corpora.toml` defining `instance-naturals-controls`,
and run:

```sh
dive-conformance --repo-root "$STAGING" golden generate --corpus instance-naturals-controls --tier all --no-sany --reason 'Pin issue 515 native-argument and configured zero-arity cache guard mutation controls with preserved startup effects' --workers 1 --jobs 2 --timeout 20 --xmx 1g --raw-invocations "$RAW"
```

`instance_naturals.rs` reads every compact case. Both profiles and requested
workers 1/4/8 are exercised on reference/core/auto, with core optimization
ablations. Output-bearing searches retain their normal worker restrictions.

| Controls | Independent obligation |
|---|---|
| `SubstLeft`, `SubstRight`, `SubstNestedArg`, `SubstPair`, `SubstPlus`, `DirectState` | Actual state dependencies, both multiplication positions and formerly working tuple/addition controls. |
| `ConfiguredState`, `ConfiguredRedirect` | A zero-arity read follows the actual configured callback/redirect, including a constant dummy body hiding a stateful replacement. |
| `ConstantInstance` | True constant instance values still get explicitly precomputed and retained; this is not a blanket INSTANCE precomputation ban. |
| `LazyUnusedActual`, `LazyUnusedEffect`, `SubstEffect` | An unused failure or print is never demanded; a demanded substituted print retains TLC's three outputs. |
| `StartupStored`, `StartupStoredRedirect` | Direct stored startup reads retain one output; the configured redirect control retains three outputs, as actually measured on the pinned clean TLC runs. |
| `LeftErrorFirst`, `RightErrorFirst` | Reached invariant failure retains underlying Assert 2132 versus division-by-zero 2179 according to multiplication argument order. Both are decided by 2111 / exit 76 with 2 distinct / 2 generated / depth 2. Fresh independent validation accepts their words and rejects an exchanged cause. |
| `AssumePriority` | False ASSUME precedes the state-dependent invariant: 2104 / exit 10, no search. Missing native assumption-name metadata is separately #504; this control claims priority/class/exit, not name agreement. |
| `NestedScopes` | Repaired reference matches TLC at 3 distinct / 4 generated / depth 3 across two nested substitutions. Existing core/auto false alarm #634 remains a separate phase finding, with no core/auto agreement credit. Its unchanged exact-main baseline word is independently invalid with the repaired reference because Inv holds at x=0. |

The first model remains strict on all three engines. The nested-scope oracle is
unchanged and strictly consumed for reference, rather than relabeling its
existing core failure as an agreement. These are selected controls, not a new
census, baseline promotion or complete INSTANCE claim.
