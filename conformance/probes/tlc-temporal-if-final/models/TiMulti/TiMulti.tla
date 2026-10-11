---- MODULE TiMulti ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live1 == [](IF x=0 THEN <>(x=1) ELSE x=2)
Live2 == [](IF x=0 THEN x=0 ELSE <>(x=1))
====
