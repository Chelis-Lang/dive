---- MODULE LetSpecification ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Spec == LET I == Init N == Next IN I /\ [][N]_x
====
