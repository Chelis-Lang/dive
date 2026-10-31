---- MODULE InitialStateRight ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x=[a |-> 0]
Next == UNCHANGED x
Inv == <<0>> # x
====
