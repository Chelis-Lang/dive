---- MODULE RoTrueRoles ----
VARIABLE x
T == TRUE
Init == x = 0
Next == x' = x
Spec == Init /\ T /\ [][Next]_x
====
