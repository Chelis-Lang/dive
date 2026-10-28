---- MODULE OlInv ----
\* In an invariant: the error is reported in the initial state (2154).
EXTENDS Naturals, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Inv == FoldLeft(LAMBDA a, y : 0, 0, <<x>>) = 0
====
