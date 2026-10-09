---- MODULE DfLazy ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
H(S) == []<>(\E b \in S : b)
P == H(IF x THEN {TRUE} ELSE {})
====
