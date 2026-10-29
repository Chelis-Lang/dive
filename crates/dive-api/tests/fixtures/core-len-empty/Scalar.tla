---- MODULE Scalar ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = 0
Next == UNCHANGED f
Inv == Len(f) = 0
====
