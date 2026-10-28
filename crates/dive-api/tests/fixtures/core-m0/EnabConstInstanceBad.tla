---- MODULE EnabConstInstanceBad ----
EXTENDS Naturals
VARIABLE x
I == INSTANCE Inner WITH C <- 2
Init == x = 0
Next == IF x < 3 THEN x' = x + 1 ELSE UNCHANGED x
Inv == IF x=1 THEN ~I!Can ELSE I!Can = (x<2)
====
---- MODULE Inner ----
EXTENDS Naturals
CONSTANT C
VARIABLE x
Step == x < C /\ x' = x + 1
Can == ENABLED Step
====
