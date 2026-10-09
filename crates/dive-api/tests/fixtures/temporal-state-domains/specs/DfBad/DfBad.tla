---- MODULE DfBad ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == <>[]({b : b \in IF x THEN {TRUE} ELSE {}}={TRUE})
====
