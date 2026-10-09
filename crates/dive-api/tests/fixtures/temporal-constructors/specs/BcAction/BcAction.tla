---- MODULE BcAction ----
VARIABLE x
vars == <<x>>
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_vars
P == []<><<({x'#x : b \in {TRUE}}={TRUE})>>_x
====
