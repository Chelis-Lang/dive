---- MODULE IfLocalArg ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
F(c) == IF c THEN x=2 ELSE x=0
Live == <>[](F(x=1))
====
