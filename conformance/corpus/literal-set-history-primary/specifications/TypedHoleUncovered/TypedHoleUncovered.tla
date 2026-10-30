---- MODULE TypedHoleUncovered ----
EXTENDS Integers, FiniteSets
CONSTANT A_x
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == x = 0 \/ (<<A_x>> \in {<<A_x>>, <<1>>})
====
