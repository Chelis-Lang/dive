---- MODULE OpInstanceBodyFails ----
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
P(v) == v
I == INSTANCE Inner WITH y <- x
Live == I!Live
====
