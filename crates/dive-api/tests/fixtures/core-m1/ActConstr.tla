---- MODULE ActConstr ----
\* ACTION_CONSTRAINT: unprimed variables read the parent, primed ones the
\* successor; a step outside it is generated and not explored.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ x' \in 0..4 /\ y' = y
        \/ y' = (y + x) % 5 /\ x' = x
Small == x' <= x + 2 /\ y' # 3
Inv == x + y < 8
====
