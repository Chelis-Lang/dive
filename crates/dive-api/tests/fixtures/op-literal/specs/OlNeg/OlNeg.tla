---- MODULE OlNeg ----
\* -1 is an operator application: passed as an operator.
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME FoldLeft(LAMBDA a, x : -1, 0, <<1>>) = -1
====
