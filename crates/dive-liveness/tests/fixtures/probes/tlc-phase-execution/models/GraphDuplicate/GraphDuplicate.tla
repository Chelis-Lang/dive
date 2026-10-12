---- MODULE GraphDuplicate ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' = 2 \/ x' = 2 \/ x' = 0 \/ x' = 1
Live == []<>(x = 2)
====
