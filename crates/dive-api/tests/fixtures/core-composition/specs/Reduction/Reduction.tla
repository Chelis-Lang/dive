---- MODULE Reduction ----
EXTENDS Integers
VARIABLES x, y
vars == <<x, y>>
Init == x = 0 /\ y = 0
First == x' \in {1, 2} /\ y' = x'
Reduction ==
    LET m == x IN
    /\ x' = x - m
    /\ y' = y + m
Next == First \cdot Reduction
Inv == x = 0 /\ y \in {0, 2, 4}
Bad == y # 4
====
