---- MODULE TLC_M0 ----
VARIABLE
    x,
    y
vars == <<x, y>>
Init == (x = TRUE) /\ (y = TRUE)
Next == (x' = ~x) /\ (y' = x)
Spec == Init /\ [][Next]_vars
Inv == ENABLED(FALSE = TRUE)
Prop == [][x' => y']_vars
====