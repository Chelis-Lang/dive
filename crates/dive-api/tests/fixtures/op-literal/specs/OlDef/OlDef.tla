---- MODULE OlDef ----
\* A named operator whose body is a numeral (2154).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
Z(a, x) == 0
ASSUME FoldLeft(Z, 0, <<1>>) = 0
====
