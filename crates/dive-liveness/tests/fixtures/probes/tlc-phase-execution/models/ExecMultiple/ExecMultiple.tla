---- MODULE ExecMultiple ----
VARIABLE x
Init == x = 0
Next == x' \in {0,1,2}
P == <>(x = 1)
Q == <>(x = 2)
====
