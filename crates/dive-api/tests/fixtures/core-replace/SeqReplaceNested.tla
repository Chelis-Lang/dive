---- MODULE SeqReplaceNested ----
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = x
Inv == x=0 /\ Len(ReplaceFirstSubSeq(<<<<9>>>>, <<<<1>>>>, <<<<1>>, <<2>>>>)) >= 0
====
