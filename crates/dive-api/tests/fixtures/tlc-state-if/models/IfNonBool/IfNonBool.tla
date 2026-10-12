---- MODULE IfNonBool ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == []<>(IF x=1 THEN x ELSE TRUE)
====
