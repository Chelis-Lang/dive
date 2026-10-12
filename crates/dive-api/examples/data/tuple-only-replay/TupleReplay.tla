---- MODULE TupleReplay ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = IF Len(Tail(<<0,1>>)) = 1 THEN 1 ELSE 2
Inv == x = 0
====
