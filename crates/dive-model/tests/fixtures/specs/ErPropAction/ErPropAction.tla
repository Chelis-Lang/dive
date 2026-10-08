---- MODULE ErPropAction ----
VARIABLE x
Init == x = 0
Next == x' = x
Spec == Init /\ [][Next]_x
Prop == [Next]_x
====
