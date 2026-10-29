---- MODULE EmptySetDomain ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = [k \in {} |-> 0]
Next == UNCHANGED f
Inv == Len(f) = 0
====
