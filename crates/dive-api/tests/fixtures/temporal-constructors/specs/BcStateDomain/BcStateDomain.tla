---- MODULE BcStateDomain ----
VARIABLE x
vars == <<x>>
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_vars
P == []<>(({b : b \in IF x THEN BOOLEAN ELSE {}}={}) <=> ~x)
====
