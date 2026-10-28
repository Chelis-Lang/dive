---- MODULE SeqReplacePartitionChoice ----
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = x
Inv == /\ x = 0
       /\ ReplaceAllSubSeqs(<<2>>, <<1,1>>, <<0,1,1,1>>) = <<0,1,2>>
====
