---- MODULE NonSequenceDomain ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = [k \in 0..1 |-> k]
Next == UNCHANGED f
Inv == Len(f) = 0
====
