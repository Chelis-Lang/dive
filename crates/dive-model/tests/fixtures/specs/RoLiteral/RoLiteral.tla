---- MODULE RoLiteral ----
VARIABLE x
Seven == 7
Init == x = 0
Next == x' = x
Spec == Init /\ Seven /\ [][Next]_x
====
