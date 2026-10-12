---- MODULE Terminal ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == UNCHANGED x
Live == <>(x=1)
====
