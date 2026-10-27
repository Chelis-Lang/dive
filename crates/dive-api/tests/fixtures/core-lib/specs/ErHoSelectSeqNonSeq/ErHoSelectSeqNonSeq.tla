---- MODULE ErHoSelectSeqNonSeq ----
\* SelectSeq of a set at x = 2: TLC 2169.
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == SelectSeq(IF x = 2 THEN {1} ELSE <<1>>, LAMBDA y : y > 0) # <<9>>
====
