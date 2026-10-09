---- MODULE BcMapGood ----
VARIABLE x
vars == <<x>>
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_vars
P == []<>({IF b THEN x ELSE ~x : b \in BOOLEAN} = BOOLEAN)
====
