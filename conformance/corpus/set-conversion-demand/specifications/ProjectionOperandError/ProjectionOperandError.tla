---- MODULE ProjectionOperandError ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == Len(SetToSeq({} \X {x \div x}))=0
====
