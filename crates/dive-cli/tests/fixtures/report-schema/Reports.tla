---- MODULE Reports ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Loop == x' = IF x < 2 THEN x + 1 ELSE x
Inv == x < 2
InvError == 2 \div (2 - x) >= 0
Spec == Init /\ [][Loop]_x
Live == <>(x = 3)
====
