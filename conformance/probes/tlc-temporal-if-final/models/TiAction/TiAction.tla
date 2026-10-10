---- MODULE TiAction ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == [](IF x'=x THEN <>(x=1) ELSE <>(x=2))
====
