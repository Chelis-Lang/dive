---- MODULE ErCmIsPrefixNonSeq ----
EXTENDS Integers, Sequences, FiniteSets, TLC, SequencesExt
ASSUME PrintT(IsPrefix(1, <<1>>))
====
