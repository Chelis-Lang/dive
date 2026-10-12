---- MODULE IfEntry ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == []((IF x=0 THEN x=0 ELSE x=1) => <>(x=2))
====
