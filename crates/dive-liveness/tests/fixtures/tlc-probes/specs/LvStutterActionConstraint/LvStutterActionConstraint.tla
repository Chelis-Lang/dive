---- MODULE LvStutterActionConstraint ----
\* TLC's implicit stutter edge bypasses ACTION_CONSTRAINT.
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x
Prop == []<>x
Changes == x' # x
====
