---- MODULE EnabConstInstanceError ----
EXTENDS Naturals
VARIABLE x
I == INSTANCE Inner WITH C <- 1 \div 0
Init == x = 0
Next == IF x < 3 THEN x' = x + 1 ELSE UNCHANGED x
Inv == I!Can
====
---- MODULE Inner ----
EXTENDS Naturals
CONSTANT C
VARIABLE x
Step == IF x = 0 THEN x' = 1 ELSE x < C /\ x' = x + 1
Can == ENABLED Step
====
