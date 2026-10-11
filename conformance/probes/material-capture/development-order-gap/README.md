# Stored ordering admission gap retained

Before the order guard, same-owned string values in a Canonical Enum set passed
TLC-mode scope admission. `R(v) == x' = (CHOOSE a \in v : TRUE)` produced two
states (`zeta`, `alpha`) instead of the matching TLC-order set's one `zeta` state.
`repro/main.rs` and its raw output retain the public caller observation. The
value helper `ops::choose` re-sorts, while the evaluator's CHOOSE uses cached
set elements; helper-only agreement would have missed this actual path.

The original 72 native positives and executable guide remain bound to their
original source archive. They do not establish cross-profile admission. The
931-test gate was stopped without source mutation: 241 passed, two active tests
interrupted, 688 unrun. Its raw log and before/after manifests remain intact.

The current guard requires matching order for stored Enum/Raw sets, records and
general functions. Tuples retain their ascending integer-domain exemption;
nested values still recurse. Public positive/negative controls cover both
profiles and all five stored representations with zero work on refusal.
The first new test failed to compile (`CheckOptions::default` is absent); its
source/log remains `order-test-compile.*`. A later empty-assert lint source/log
remains `order-test-clippy.*`. Neither earns runtime credit.
