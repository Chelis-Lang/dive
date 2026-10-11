---- MODULE ConfiguredSpecification ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Unproved == <> (x = 3)
Selected == Init /\ [][Next]_x
====
