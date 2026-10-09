---- MODULE QtEmpty ----
CONSTANT S
VARIABLE x
Init == x = 0
Next == x' = x
P == \E i \in S: []<>(x=i)
====
