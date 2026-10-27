---- MODULE ErCmAllSubSeqsNonSeq ----
\* AllSubSeqs of a set at x = 2: TLC 2283.
EXTENDS Naturals, SequencesExt
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == AllSubSeqs(IF x = 2 THEN {1} ELSE <<1>>) # {}
====
