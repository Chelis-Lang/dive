---- MODULE CpRecord ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x = 0 /\ x' = {i : i \in 0..65536}
====
