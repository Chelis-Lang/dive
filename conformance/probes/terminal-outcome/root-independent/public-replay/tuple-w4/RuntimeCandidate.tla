---- MODULE RuntimeCandidate ----
EXTENDS Integers,TLC
VARIABLES x,s
Init == x=(0) /\ s = <<"literal",0>>
Next == x'=(IF x=0 THEN 1 ELSE 1) /\ s'=s
Live == <>(x=2)
Hook == TRUE
====
