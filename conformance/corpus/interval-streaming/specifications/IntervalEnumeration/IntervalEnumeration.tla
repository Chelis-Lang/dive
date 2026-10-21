---- MODULE IntervalEnumeration ----
EXTENDS Integers
VARIABLE x
Init == x \in -2..2
Next == x' \in -2..2
Inv == TRUE
====
