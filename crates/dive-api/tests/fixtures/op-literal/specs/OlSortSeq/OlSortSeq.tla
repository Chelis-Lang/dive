---- MODULE OlSortSeq ----
\* SortSeq reports the value as its second argument (2169).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME SortSeq(<<1, 2>>, LAMBDA a, b : 0) = <<1, 2>>
====
