---- MODULE SeqReplaceMixed ----
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = x
Inv == x=0 /\ Len(ReplaceAllSubSeqs(<<1>>, <<1>>, <<1,"a">>)) >= 0
====
