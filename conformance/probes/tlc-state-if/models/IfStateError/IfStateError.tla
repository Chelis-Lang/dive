---- MODULE IfStateError ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == []<>(IF x=1 THEN 1 \div 0 = 0 ELSE x=2)
====
