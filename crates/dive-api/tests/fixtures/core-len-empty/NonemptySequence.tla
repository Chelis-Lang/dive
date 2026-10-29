---- MODULE NonemptySequence ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = [k \in 1..2 |-> k]
Next == UNCHANGED f
Inv == Len(f) = 2
====
