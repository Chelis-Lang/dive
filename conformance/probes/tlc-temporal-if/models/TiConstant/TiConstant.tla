---- MODULE TiConstant ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == [](IF TRUE THEN <>(x=1) ELSE <>(x=2))
====
