---- MODULE ErHoInBody ----
\* A division by zero inside a SelectSeq test at x = 3.
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == SelectSeq(<<1, 2>>, LAMBDA y : 6 \div (3 - x) > y) # <<9>>
====
