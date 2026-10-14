---- MODULE SimChoicePrinted ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in {0, 1}
A == x' \in Print("domain", {0, 2, 4})
B == x' = Print("step", x + 1)
Disabled == FALSE
Next == A \/ Disabled \/ B
====
