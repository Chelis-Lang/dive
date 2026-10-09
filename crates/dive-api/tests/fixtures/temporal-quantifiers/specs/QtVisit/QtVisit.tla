---- MODULE QtVisit ----
CONSTANT S
VARIABLE x
Init == x = 0
Next == x' \in S
P == \A i \in S: []<>(x=i)
====
