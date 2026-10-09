---- MODULE TLC_M0 ----
VARIABLE
    x,
    y
vars == <<x, y>>
x2 == INSTANCE TLC_x1
Init == (x = TRUE) /\ (y = TRUE)
Next == (x' = ~x) /\ (y' = x2!x0)
Spec == Init /\ [][Next]_vars
Inv == x => y
Prop == [][x' => y']_vars
====