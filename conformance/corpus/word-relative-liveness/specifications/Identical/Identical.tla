---- MODULE Identical ----
VARIABLE x
Init == x=0
Next == UNCHANGED x
P == <>(x=1)
Q == <>(x=1)
====
