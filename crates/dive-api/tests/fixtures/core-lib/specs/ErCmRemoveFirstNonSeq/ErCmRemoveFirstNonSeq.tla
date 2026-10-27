---- MODULE ErCmRemoveFirstNonSeq ----
\* RemoveFirst of a string at x = 3: TLC 2169.
EXTENDS Naturals, SequencesExt, Functions
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == RemoveFirst(IF x = 3 THEN "ab" ELSE <<1>>, 1) # <<9>>
====
