---- MODULE OpExportedFormalHolds ----
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
P(v) == v
I == INSTANCE Inner WITH y <- x
Live == I!Use(P)
====
