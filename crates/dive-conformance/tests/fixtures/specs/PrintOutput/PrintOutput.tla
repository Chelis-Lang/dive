---- MODULE PrintOutput ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ PrintT(<<"step", x>>) /\ x' = x + 1
====
