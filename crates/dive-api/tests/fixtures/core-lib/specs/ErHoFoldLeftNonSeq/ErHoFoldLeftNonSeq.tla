---- MODULE ErHoFoldLeftNonSeq ----
\* FoldLeft over an integer at x = 1: TLC 2169.
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == FoldLeft(LAMBDA a, y : a + y, 0, IF x = 1 THEN 4 ELSE <<1>>) > 0
====
