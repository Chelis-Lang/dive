---- MODULE Terminal ----
EXTENDS Integers
VARIABLE x
Init == x \in {}
Next == x'=(IF x=0 THEN 1 ELSE 1)
Live == <>(x=1)
====
