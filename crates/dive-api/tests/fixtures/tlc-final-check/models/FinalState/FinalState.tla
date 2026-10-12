---- MODULE FinalState ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = (x+1)%3
Live == <>[](x = 0)
====
