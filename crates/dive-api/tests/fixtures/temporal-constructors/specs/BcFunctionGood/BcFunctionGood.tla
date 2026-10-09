---- MODULE BcFunctionGood ----
VARIABLE x
vars == <<x>>
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_vars
P == []<>([b \in BOOLEAN |-> IF b THEN x ELSE ~x][TRUE]=x)
====
