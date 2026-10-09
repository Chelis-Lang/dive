---- MODULE DfFunction ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
F == [b \in IF x THEN {TRUE} ELSE {} |-> b /\ x]
P == []<>(DOMAIN F={TRUE})
====
