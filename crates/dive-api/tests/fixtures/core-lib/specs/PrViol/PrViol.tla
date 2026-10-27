---- MODULE PrViol ----
\* Print with an invariant violation: TLC prints again while it rebuilds
\* the trace, which the core does not reproduce yet: it refuses.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == /\ PrintT(<<"next", x>>)
        /\ x' = x + 1
Inv == x < 2
====
