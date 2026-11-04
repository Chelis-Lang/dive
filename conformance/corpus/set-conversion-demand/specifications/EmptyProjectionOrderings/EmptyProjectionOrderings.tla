---- MODULE EmptyProjectionOrderings ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET s == {x,TRUE} p == {} \X s IN /\ SetToSeqs(p) = {<<>>} /\ x \in s
====
