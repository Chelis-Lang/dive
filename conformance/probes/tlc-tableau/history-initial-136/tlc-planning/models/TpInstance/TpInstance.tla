---- MODULE TpInstance ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Flip == x' = ~x
Spec == Init /\ [][Next]_x
I == INSTANCE TpPart WITH y <- x
Identity(p) == p
Live == Identity(I!Claim)

====
