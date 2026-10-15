---- MODULE NestedEnabled ----
EXTENDS Naturals
CONSTANT K(_)
VARIABLE x
Init==x=0
Next==x'=1
Q==ENABLED (x'=1)
Use(p)==p
Ignore(p)==TRUE
QueryTarget(p)==ENABLED (x'=1)
Known==K(Q) /\ x'=1
Lazy==LET held==Q IN K(held) /\ x'=1
Unused==K(Ignore(Q)) /\ x'=1
Raw(p)==K(p) /\ x'=1
====
