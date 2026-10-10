---- MODULE TpStrong ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x /\ SF_x(Flip)
Live == <>[](x = TRUE)

====
