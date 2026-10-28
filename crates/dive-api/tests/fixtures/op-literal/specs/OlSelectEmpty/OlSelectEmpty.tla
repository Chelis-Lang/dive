---- MODULE OlSelectEmpty ----
\* SelectSeq of the empty sequence does not look at its operator.
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME SelectSeq(<<>>, LAMBDA x : 0) = <<>>
====
