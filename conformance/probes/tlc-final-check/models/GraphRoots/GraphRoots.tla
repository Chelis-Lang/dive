---- MODULE GraphRoots ----
EXTENDS Integers, TLC
VARIABLE x
Init == x \in {0,1,2}
Next == x' \in {0,1,2}
Live == []<>(x = 2)
====
