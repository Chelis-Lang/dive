---- MODULE ExistsSetOrder ----
\* Probe (with InSetOrder): \E enumerates its bound in normalized order
\* (Tool.contexts uses Ordering.NORMALIZED), so although the literal is
\* {3, 1, 2}, the first successor is 1 and the counterexample ends in 1 after
\* 2 generated states.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \E v \in {3, 1, 2} : x' = v
Inv == x \notin {1, 3}
====
