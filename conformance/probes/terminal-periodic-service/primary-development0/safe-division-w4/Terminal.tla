---- MODULE Terminal ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=(IF x=0 THEN 1 ELSE 1)
Live == <>(x=1)
Hook == 1 \div 0
====
