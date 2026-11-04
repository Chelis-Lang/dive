---- MODULE NonemptyProjection ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET s == {x+1,x,x+1} p == {x} \X s IN SetToSeq(p) = << <<x,x>>, <<x,x+1>> >>
====
