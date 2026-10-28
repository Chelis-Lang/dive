---- MODULE EnabHiddenOperatorState ----
EXTENDS Naturals
VARIABLES x,y
Inc(v) == v+1
I == INSTANCE Middle WITH hidden <- y
Init == x=0 /\ y=0
Next == IF x<3 THEN x'=x+1 /\ UNCHANGED y ELSE UNCHANGED <<x,y>>
Inv == I!Can = (x<2)
====
---- MODULE Middle ----
EXTENDS Naturals
VARIABLES x,y,hidden
Inc(v) == v+hidden
J == INSTANCE Inner WITH C <- 2, Inc <- Inc
Can == J!Can
====
---- MODULE Inner ----
EXTENDS Naturals
CONSTANTS C, Inc(_)
VARIABLES x,y
Step == y'=y+1 /\ x<C /\ x'=Inc(x)
Can == ENABLED Step
====
