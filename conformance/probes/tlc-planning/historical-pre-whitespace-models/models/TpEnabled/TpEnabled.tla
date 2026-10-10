---- MODULE TpEnabled ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Flip)
Identity(p) == p
Live == Identity([]<>ENABLED Flip)

====
