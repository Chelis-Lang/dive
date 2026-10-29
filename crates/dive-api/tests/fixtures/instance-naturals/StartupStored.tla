---- MODULE StartupStored ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
CONSTANT C
Stored == Print(42,6)
Inv == Stored = 6 /\ C = 6 /\ C = 6
====
