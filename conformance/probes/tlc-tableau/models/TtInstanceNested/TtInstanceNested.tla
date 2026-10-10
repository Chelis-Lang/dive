---- MODULE TtInstanceNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
I == INSTANCE TtPart WITH p <- x
Live == I!Claim
====
