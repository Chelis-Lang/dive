---- MODULE EnabOperatorState ----
EXTENDS Naturals
VARIABLES x,y
Inc(v) == v+y
I == INSTANCE Inner WITH C <- 2, Inc <- Inc
Init == x=0 /\ y=0
Next == IF x<3 THEN x'=x+1 /\ UNCHANGED y ELSE UNCHANGED <<x,y>>
Inv == I!Can = (x<2)
====
---- MODULE Inner ----
EXTENDS Naturals
CONSTANTS C, Inc(_)
VARIABLES x,y
Step == y'=y+1 /\ x<C /\ x'=Inc(x)
Can == ENABLED Step
====
