---- MODULE Temporal ----
VARIABLE x
Init == x=0
Next == x'=x
P == []<>(x=1)
Q == []<>(x=0)
====
