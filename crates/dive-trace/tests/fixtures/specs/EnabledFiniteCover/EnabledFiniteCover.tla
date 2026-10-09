---- MODULE EnabledFiniteCover ----
EXTENDS Integers
VARIABLES x, y
Init == x \in (-1)..4 /\ y \in (-1)..4
Next == UNCHANGED <<x, y>>
Relation == x' \in 0..2 /\ y' \in 0..2 /\ x' + y' = x + y
Checks == (ENABLED Relation) <=> (x + y >= 0 /\ x + y <= 4)
====
