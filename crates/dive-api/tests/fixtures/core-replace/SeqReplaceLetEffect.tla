---- MODULE SeqReplaceLetEffect ----
EXTENDS Naturals, Sequences, SequencesExt, TLC
VARIABLE x
Init == x = 0
Next == x < 1 /\ x' = x + 1
Inv == LET r == IF PrintT(<<"r", x>>) THEN <<9>> ELSE <<9>> IN ReplaceFirstSubSeq(r, <<1>>, <<1>>) = <<9>>
====
