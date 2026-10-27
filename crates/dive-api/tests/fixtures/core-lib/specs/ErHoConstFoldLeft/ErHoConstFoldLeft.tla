---- MODULE ErHoConstFoldLeft ----
\* TLC cannot apply an operator with a constant body passed to FoldLeft
\* (error 2154); the core refuses the call.
EXTENDS Naturals, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Inv == FoldLeft(LAMBDA a, y : 0, 0, <<x>>) = 0
====
