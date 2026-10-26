---- MODULE LazyCup ----
\* x' \in a union TLC keeps lazy whose parts are disjoint: its elements()
\* lists every element once, so the core enumerates it. Unions of set
\* enumerations are computed eagerly.
EXTENDS Naturals
VARIABLES r, n
Init == r = [a |-> 0] /\ n \in {1, 2} \cup {2, 3}
Next == /\ r' \in [a : {1}] \cup [a : {2, 3}] \cup {[a |-> n + 10]}
        /\ n' \in UNION {{n + 5}, 1..2}
        /\ n' \in {k \in (1..3) \cup {4} : k > 1} \cup {}
====
