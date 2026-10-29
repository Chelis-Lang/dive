---- MODULE EmptyInterval ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = [k \in 1..0 |-> 0]
Next == UNCHANGED f
Inv == Len(f) = 0
====
