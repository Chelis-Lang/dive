---- MODULE OlFoldLeft ----
\* A LAMBDA whose body is a numeral reaches FoldLeft as its value (2154).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME FoldLeft(LAMBDA a, x : 0, 0, <<1>>) = 0
====
