---- MODULE NextStateLeft ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x = <<0>>
Next == x # [a |-> 0] /\ UNCHANGED x
Inv == TRUE
====
