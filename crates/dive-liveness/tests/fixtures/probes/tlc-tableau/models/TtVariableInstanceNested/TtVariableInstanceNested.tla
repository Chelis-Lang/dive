---- MODULE TtVariableInstanceNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
I == INSTANCE TtVariablePart WITH p <- x
Live == I!Claim
====
