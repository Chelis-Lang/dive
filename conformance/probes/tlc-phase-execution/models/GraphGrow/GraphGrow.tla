---- MODULE GraphGrow ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' \in 0..18
Live == []<>(x = 18)
====
