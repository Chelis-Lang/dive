---- MODULE NxInRcdOrder ----
\* Probe: x' \in [a : S, b : T] enumerates records with the last field
\* (in TLC's field order) varying fastest; the invariant fails at the
\* first record with b = 2.
VARIABLE r
Init == r = [a |-> 0, b |-> 0]
Next == r' \in [a : {1, 2}, b : {1, 2}]
Inv == r.b # 2
====
