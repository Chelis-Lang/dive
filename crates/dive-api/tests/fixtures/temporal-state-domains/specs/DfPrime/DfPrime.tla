---- MODULE DfPrime ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == []<><<((\E b \in IF x THEN {TRUE} ELSE {} : b)')>>_x
====
