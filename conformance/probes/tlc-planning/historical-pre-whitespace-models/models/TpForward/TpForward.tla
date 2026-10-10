---- MODULE TpForward ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x 
Identity(p) == p
Live == Identity(<>[](x = TRUE))

====
