---- MODULE TpWeak ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Flip)
Live == []<>(x = TRUE)

====
