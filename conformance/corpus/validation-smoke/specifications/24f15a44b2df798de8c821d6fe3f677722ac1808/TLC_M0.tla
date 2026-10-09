---- MODULE TLC_M0 ----
VARIABLE
    x,
    y
vars == <<x, y>>
Init == (x = TRUE) /\ (y = TRUE)
Next == (x' = ~x) /\ (y' = (FALSE = FALSE))
Spec == Init /\ [][Next]_vars
Inv == x => y
Prop == [][x' => y']_vars
====