---- MODULE TiForward ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
Gate(p) == IF x=0 THEN p ELSE x=1
Live == [](Gate(<>(x=2)))
====
