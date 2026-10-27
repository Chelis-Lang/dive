---- MODULE OlSelectSeq ----
\* SelectSeq reports the value as its second argument (2266).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME SelectSeq(<<1>>, LAMBDA x : 0) = <<>>
====
