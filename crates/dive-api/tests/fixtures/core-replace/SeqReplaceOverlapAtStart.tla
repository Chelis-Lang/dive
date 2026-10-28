---- MODULE SeqReplaceOverlapAtStart ----
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = x
Inv == /\ x = 0
       /\ ReplaceAllSubSeqs(<<9>>, <<1,1>>, <<1,1,1>>) = <<9,1>>
====
