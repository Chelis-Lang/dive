---- MODULE TLC_M0 ----
VARIABLE
    x,
    y
vars == <<x, y>>
Init == (x = TRUE) /\ (y = TRUE)
Next == (x' = ~x) /\ (y' = (
    LET
        LocalD0 == TRUE
    IN LocalD0))
Spec == Init /\ [][Next]_vars
Inv == x => y
Prop == [][x' => y']_vars
====