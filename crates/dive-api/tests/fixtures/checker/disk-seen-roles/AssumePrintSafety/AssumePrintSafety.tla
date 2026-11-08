---- MODULE AssumePrintSafety ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
ASSUME PrintT("must-not-assume")
Spec == Init /\ [][Next]_x
====
