---- MODULE ReceiverAfterProjection ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET s == {x,TRUE} p == {} \X s c == [edge |-> p] IN /\ Len(SetToSeq(c.edge))=0 /\ ~(x \in c.edge)
====
