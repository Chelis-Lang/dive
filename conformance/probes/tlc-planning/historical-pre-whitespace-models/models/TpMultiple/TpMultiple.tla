---- MODULE TpMultiple ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Flip)
P == []<>(x = TRUE)
Q == <>[](x = FALSE)
R == <>(x = TRUE)

====
