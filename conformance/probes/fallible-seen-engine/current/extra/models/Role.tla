---- MODULE Role ----
EXTENDS TLC
VARIABLE x
Unused == PrintT("must not bind")
ASSUME PrintT("must not assume")
Init == x=0
Next == x'=x
Spec == Init /\ [][Next]_x
P == []<>(x=0)
====
