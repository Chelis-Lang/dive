---- MODULE Terminal ----
EXTENDS Integers
VARIABLE x
Init == FALSE
Next == x'=(IF x=0 THEN 1 ELSE 1)
Live == <>(x=1)
Hook == FALSE
====
