---- MODULE StringReplaceInlineEffect ----
EXTENDS Naturals, Sequences, SequencesExt, TLC
VARIABLE x
Init == x = 0
Next == x < 1 /\ x' = x + 1
Inv == ReplaceFirstSubSeq(IF PrintT(<<"r", x>>) THEN "z" ELSE "z", IF PrintT(<<"s", x>>) THEN "a" ELSE "a", IF PrintT(<<"t", x>>) THEN "a" ELSE "a") = "z"
====
