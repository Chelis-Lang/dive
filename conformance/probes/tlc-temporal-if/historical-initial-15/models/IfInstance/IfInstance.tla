---- MODULE IfInstance ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
---- MODULE Inner ----
VARIABLE y
P == IF y=1 THEN y=2 ELSE y=0
====
I == INSTANCE Inner WITH y <- x
Live == <>[](I!P)
====
