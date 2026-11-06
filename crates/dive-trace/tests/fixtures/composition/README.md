# Bounded composition oracle controls

`Reduction.tla` is the complete finite first stage and deterministic LET tail
used by the Core composition control. Pinned TLC succeeds with 13 generated,
3 distinct states and depth 2; default mathematical counting generates 7.
The independent test brute-forces its finite relation and checks both real
successors, the opposite invariant claim, a forged row and a wrong Init.

`ReplayComposition.tla` isolates the continuation artifact: the actual relation
of `A` contains only the final edge to zero, while TLC applies the changed
subscript to the two intermediate steps. `facts.cfg` succeeds; `weak.cfg` and
`strong.cfg` each report a one-state stuttering violation. The mathematical
nonstuttering relation at zero is empty, so both fairness kinds hold. These
exact differences are D-022. `oracle.json` retains the pinned observations;
raw output is regenerated rather than committed.

Reproduce each config with the pinned oracle `tlc-2026.10.06.014338` (jar SHA-256
in `oracle.json`), its pinned JRE, and this command from the fixture directory:

```sh
java -XX:+UseParallelGC -Xmx1g -Xss8m \
  -Dtlc2.tool.impl.Tool.cdot=true -cp "$PINNED_TLA2TOOLS_JAR" \
  tlc2.TLC ReplayComposition.tla -config facts.cfg \
  -workers 1 -lncheck final -cleanup -tool -fp 0 -seed 0 \
  -metadir "$OUTSIDE_REPO_STATES"
```

Repeat for `weak.cfg` and `strong.cfg`. For `Reduction.tla`, use `Reduction.cfg`.
`cargo test -p dive-trace --test composition --locked` consumes these fixtures;
source-error, interruption and cold/warm controls use the existing adjacent tests.
