---- MODULE PrintedSpecification ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Startup == PrintT("must-not-start")
Spec == Init /\ [][Next]_x
====
