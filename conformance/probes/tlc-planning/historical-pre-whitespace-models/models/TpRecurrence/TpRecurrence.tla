---- MODULE TpRecurrence ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x 
Live == []<>(x = TRUE)

====
