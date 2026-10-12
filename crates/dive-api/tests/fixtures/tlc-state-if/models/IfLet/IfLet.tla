---- MODULE IfLet ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == <>[](LET G(c) == IF c THEN x=2 ELSE x=0 IN G(x=1))
====
