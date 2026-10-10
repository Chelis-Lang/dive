---- MODULE TpLocal ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x 
Identity(p) == p
Live == LET Alias(q) == Identity(q) IN Alias([]<>(x = TRUE))

====
