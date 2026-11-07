---- MODULE NextReadSub ----
EXTENDS Naturals
VARIABLE x
I == INSTANCE NextReadSubM WITH c <- x'
Op(a) == \/ (x' = 0 /\ a = 0) \/ (x' = 1 /\ a = 1)
Init == x = 0
Next == Op(I!R(1))
====
