---- MODULE BcFilterGood ----
VARIABLE x
vars == <<x>>
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_vars
P == []<>({b \in BOOLEAN : b=x} = {x})
====
