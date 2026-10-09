---- MODULE DfEnabled ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == <>[]ENABLED(\E b \in IF x THEN {TRUE} ELSE {} : x'=b)
====
