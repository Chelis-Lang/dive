---- MODULE ErPropActionPlain ----
VARIABLE x
Init == x = 0
Next == x' = x
Spec == Init /\ [][Next]_x
Prop == x' = x
====
