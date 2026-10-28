---- MODULE OlConst ----
\* A declared constant body is passed as an operator.
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME FoldLeft(LAMBDA a, x : N, 0, <<1>>) = 3
====
