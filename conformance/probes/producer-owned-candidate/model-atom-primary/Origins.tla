---- MODULE Origins ----
EXTENDS Integers
CONSTANT C
VARIABLES x,s
Init == x=0 /\ s=C
Next == x'=1 /\ s'=s
Full == <>(x=2) /\ <>(x=3)
Hook == TRUE
====
