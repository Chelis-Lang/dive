---- MODULE EnabConfiguredHiddenState ----
EXTENDS Naturals
VARIABLES x,y
CONSTANT Inc(_)
H == INSTANCE Hidden WITH hidden <- y
ActualInc(v) == H!Shift(v)
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

---- MODULE Hidden ----
EXTENDS Naturals
VARIABLE hidden
Shift(v) == v+hidden
====
