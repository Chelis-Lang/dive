---- MODULE Terminal ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=(1 \div 0)
Live == <>(x=1)
Hook == FALSE
====
