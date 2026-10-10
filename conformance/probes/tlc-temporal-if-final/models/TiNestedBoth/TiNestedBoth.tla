---- MODULE TiNestedBoth ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == [](IF x=0 THEN (IF x=1 THEN <>(x=1) ELSE <>(x=2)) ELSE <>(x=2))
====
