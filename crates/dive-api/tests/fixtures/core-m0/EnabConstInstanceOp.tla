---- MODULE EnabConstInstanceOp ----
EXTENDS Naturals
VARIABLE x
Inc(v) == v + 1
I == INSTANCE Inner WITH C <- 2, Inc <- Inc
Init == x = 0
Next == IF x < 3 THEN x' = x + 1 ELSE UNCHANGED x
Inv == I!Can = (x < 2)
====
---- MODULE Inner ----
EXTENDS Naturals
CONSTANT C, Inc(_)
VARIABLE x
Step == x < C /\ x' = Inc(x)
Can == ENABLED Step
====
