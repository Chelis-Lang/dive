# Captured source representations and error endings

`Record.cfg` reproduces the generator-error replay regression in candidate
`6eaf5ce1` (#626). The minimal model uses `Init`, `Next` and `P` above. A fresh
pinned TLC run during independent review (jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`)
reported `TLC_MODULE_DIVISION_BY_ZERO` (2179), exit 255, two generated and two
distinct states, depth two, with a two-state error word. The reference agreed;
Core and Auto instead reported internal error because the captured original
source was compared with replay's canonical identity.

The tuple and nested variants exercise other retained view classes; the general
function and ordinary safety configurations are negative controls for the
original/canonical distinction. Initial failure, deadlock, invariant violation
and invariant-error configurations cover the other trace-ending constructors
and the call-stack replay transport. Tests compare their actual reports and
words with separately executed reference runs in both profiles. These controls
are not newly generated oracle goldens.
