---- MODULE EnabConstInstanceLazy ----
EXTENDS Naturals
VARIABLE x
I == INSTANCE Inner WITH C <- 1 \div 0
Init == x = 0
Next == IF x < 3 THEN x' = x + 1 ELSE UNCHANGED x
Inv == ~I!Can
====
---- MODULE Inner ----
EXTENDS Naturals
CONSTANT C
VARIABLE x
Step == FALSE /\ x' = C
Can == ENABLED Step
====
