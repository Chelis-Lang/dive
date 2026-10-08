---- MODULE ErCmReplaceFallbackError ----
EXTENDS Integers, Sequences, FiniteSets, TLC, SequencesExt
ASSUME PrintT(ReplaceAllSubSeqs(<<1>>, 3, <<1>>))
====
