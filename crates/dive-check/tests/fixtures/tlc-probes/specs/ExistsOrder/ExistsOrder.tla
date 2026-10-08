---- MODULE ExistsOrder ----
\* Probe: the order in which TLC enumerates \E x \in S, y \in T. The first of
\* 12 and 21 to be generated violates Inv and ends the counterexample. If the
\* last bound variable varies fastest the order is 11, 12, 21, 22 and the
\* counterexample ends in 12 after 3 generated states; if the first varies
\* fastest, it ends in 21 after 3 generated states.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \E i \in {1, 2}, j \in {1, 2} : x' = 10 * i + j
Inv == x \notin {12, 21}
====
