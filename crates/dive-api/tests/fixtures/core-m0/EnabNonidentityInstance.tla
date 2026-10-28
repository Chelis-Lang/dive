---- MODULE EnabNonidentityInstance ----
EXTENDS Naturals
VARIABLES x,y
I == INSTANCE Inner WITH x <- y, y <- y
Init == x=0 /\ y=1
Next == UNCHANGED <<x,y>>
Inv == I!Can
====
---- MODULE Inner ----
EXTENDS Naturals
VARIABLES x,y
Step == x'=0 /\ y'=1
Can == ENABLED Step
====
