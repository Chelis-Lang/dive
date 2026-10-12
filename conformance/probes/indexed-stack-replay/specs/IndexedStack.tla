---- MODULE IndexedStack ----
EXTENDS Naturals, Sequences, TLC
VARIABLES x, stack
Owners == {1,2}
Init == x=0 /\ stack = [self \in Owners |-> <<0,1>>]
Next == (x=0 /\ x'=1 /\ stack'=[stack EXCEPT ![1]=Tail(stack[1])])
        \/ (x=1 /\ x'=2 /\ stack'=[stack EXCEPT ![2]= <<4>> \o stack[2]])
        \/ (x>1 /\ UNCHANGED <<x,stack>>)
Inv == x<2
====
