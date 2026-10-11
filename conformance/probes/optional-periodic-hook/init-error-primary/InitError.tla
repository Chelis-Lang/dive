---- MODULE InitError ----
EXTENDS Integers
VARIABLE x
Init == x=(1 \div 0)
Next == x'=1
Live == <>(x=2)
====
