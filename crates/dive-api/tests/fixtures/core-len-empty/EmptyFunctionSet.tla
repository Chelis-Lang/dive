---- MODULE EmptyFunctionSet ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = CHOOSE q \in [1..0 -> {0}] : TRUE
Next == UNCHANGED f
Inv == Len(f) = 0
====
