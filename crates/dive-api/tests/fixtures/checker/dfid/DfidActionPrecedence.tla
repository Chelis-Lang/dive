---- MODULE DfidActionPrecedence ----
EXTENDS Naturals
VARIABLES x,y
Init == x = 0 /\ y = 0
Next == \E i \in {0,1}: IF i = 0 THEN x' = 1 ELSE (x' = 0 /\ y' = 1 \div x)
====
