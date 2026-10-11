---- MODULE Material ----
EXTENDS Integers
VARIABLES x,s
Init == x=0 /\ s=<<"literal",0>>
Next == x'=1 /\ s'=s
Live == <>(x=2)
Hook == TRUE
====
