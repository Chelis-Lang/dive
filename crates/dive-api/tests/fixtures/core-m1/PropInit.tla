---- MODULE PropInit ----
\* An implied initial predicate false on the second initial state.
VARIABLE x
Init == x \in {0, 1}
Next == x' = x
Start == x = 0
====
