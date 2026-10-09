---- MODULE BcEmpty ----
VARIABLE x
vars == <<x>>
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_vars
P == []<>({x : b \in {}}={})
====
