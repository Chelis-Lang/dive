---- MODULE PhaseLiveGet ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' \in {0,1,2}
Live == []<>(TLCGet("level") = 1)
====
