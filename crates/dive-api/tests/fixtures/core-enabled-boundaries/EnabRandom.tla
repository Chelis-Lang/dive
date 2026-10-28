---- MODULE EnabRandom ----
EXTENDS Naturals, TLC
VARIABLES x,y
Inc(v) == v+RandomElement({1})
I == INSTANCE Inner WITH C <- 2, Inc <- Inc
Init == x=0 /\ y=0
Next == IF x<3 /\ I!Can THEN x'=x+1 /\ UNCHANGED y ELSE UNCHANGED <<x,y>>
Inv == x<=2
====
---- MODULE Inner ----
EXTENDS Naturals, TLC
CONSTANTS C, Inc(_)
VARIABLES x,y
Step == y'=y+1 /\ x<C /\ x'=Inc(x)
Can == ENABLED Step
====
