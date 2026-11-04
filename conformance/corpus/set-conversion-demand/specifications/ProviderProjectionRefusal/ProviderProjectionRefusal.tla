---- MODULE ProviderProjectionRefusal ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET p == Seq({x}) \X {} IN Len(SetToSeq(p))=0
====
