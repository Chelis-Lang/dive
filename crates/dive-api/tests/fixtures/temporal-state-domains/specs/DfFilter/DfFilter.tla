---- MODULE DfFilter ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == <>[]({b \in IF x THEN BOOLEAN ELSE {} : b}={TRUE})
====
