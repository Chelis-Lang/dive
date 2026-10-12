---- MODULE TiNested ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == [](IF x=0 THEN (IF x=1 THEN <>(x=2) ELSE x=0) ELSE x=1)
====
