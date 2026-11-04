---- MODULE NestedIntervalProjection ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET s == {x,TRUE} p == ((x..(x-1)) \X s) \X {x} IN /\ Len(SetToSeq(p))=0 /\ x \in s
====
