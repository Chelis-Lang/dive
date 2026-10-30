---- MODULE Mutual ----
VARIABLE x
Init == x=0
Next == (x=0 /\ x' \in {1,2}) \/ (x # 0 /\ UNCHANGED x)
P == <>(x=1)
Q == <>(x=2)
====
