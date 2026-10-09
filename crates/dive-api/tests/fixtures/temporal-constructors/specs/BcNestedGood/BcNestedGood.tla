---- MODULE BcNestedGood ----
VARIABLE x
vars == <<x>>
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_vars
P == []<>([b \in BOOLEAN |-> {IF c THEN x ELSE ~x : c \in BOOLEAN}][TRUE]=BOOLEAN)
====
