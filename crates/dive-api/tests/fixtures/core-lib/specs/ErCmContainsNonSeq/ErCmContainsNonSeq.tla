---- MODULE ErCmContainsNonSeq ----
\* Contains of a set at x = 2: TLC 2283.
EXTENDS Naturals, SequencesExt, Functions
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == Contains(IF x = 2 THEN {1} ELSE <<1>>, 1)
====
