---- MODULE EmptyIntervalOtherStart ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = [k \in 5..3 |-> 0]
Next == UNCHANGED f
Inv == Len(f) = 0
====
