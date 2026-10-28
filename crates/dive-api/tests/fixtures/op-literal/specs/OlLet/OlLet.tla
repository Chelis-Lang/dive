---- MODULE OlLet ----
\* A LET operator whose body is a numeral (2154).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME LET Z(a, x) == 0 IN FoldLeft(Z, 0, <<1>>) = 0
====
