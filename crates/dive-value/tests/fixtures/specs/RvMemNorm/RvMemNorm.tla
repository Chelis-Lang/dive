---- MODULE RvMemNorm ----
EXTENDS Integers, TLC
VARIABLE x, i
Init == x = {[qfa |-> 1, qfb |-> 1], [qfa |-> 2, qfb |-> 2]} /\ i = 0
Next == i = 0 /\ i' = 1 /\ PrintT(<<1>> \in x) /\ UNCHANGED x
====
