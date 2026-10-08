---- MODULE ErSubstNonConst ----
EXTENDS Naturals
CONSTANT A
VARIABLE x
D == x + 1
Init == x = 0
Next == x' = A
====
