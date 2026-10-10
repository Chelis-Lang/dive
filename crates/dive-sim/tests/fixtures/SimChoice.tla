---- MODULE SimChoice ----
EXTENDS Naturals
VARIABLE x
Init == x \in {0, 1}
A == x' \in {0, 2, 4}
B == x' = x + 1
Disabled == FALSE
Next == A \/ Disabled \/ B
====
