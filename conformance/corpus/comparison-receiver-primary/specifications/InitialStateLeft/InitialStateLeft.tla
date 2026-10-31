---- MODULE InitialStateLeft ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x = <<0>>
Next == UNCHANGED x
Inv == x # [a |-> 0]
====
