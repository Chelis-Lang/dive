---- MODULE TiLet ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Live == LET Gate(p) == IF x=0 THEN p ELSE <>(x=1) IN [](Gate(<>(x=2)))
====
