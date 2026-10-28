---- MODULE OlTrue ----
\* TRUE is an operator application: passed as an operator.
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME FoldLeft(LAMBDA a, x : TRUE, TRUE, <<1>>)
====
