---- MODULE SeqReplaceAllTrace ----
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE s
Init == s = <<0,1,0>>
Next == s' = ReplaceAllSubSeqs(<<1>>, <<0>>, s)
Inv == s # <<1,1,1>>
====
