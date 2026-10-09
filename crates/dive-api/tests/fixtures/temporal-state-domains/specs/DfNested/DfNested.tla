---- MODULE DfNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == []<>(\A b \in IF x THEN {TRUE} ELSE {} : (\E c \in IF b THEN {x} ELSE {} : c))
====
