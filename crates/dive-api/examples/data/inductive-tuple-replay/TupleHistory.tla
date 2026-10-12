---- MODULE TupleHistory ----
EXTENDS Naturals, Sequences, TLC
VARIABLES x,s

Init == x=0 /\ s = <<0,1>>
Next == x'=1 /\ s'=Tail(s)
Inv == x=0
====
