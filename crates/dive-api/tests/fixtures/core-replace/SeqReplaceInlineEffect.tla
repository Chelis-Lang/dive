---- MODULE SeqReplaceInlineEffect ----
EXTENDS Naturals, Sequences, SequencesExt, TLC
VARIABLE x
Init == x = 0
Next == x < 1 /\ x' = x + 1
Inv == ReplaceFirstSubSeq(IF PrintT(<<"r", x>>) THEN <<9>> ELSE <<9>>, IF PrintT(<<"s", x>>) THEN <<1>> ELSE <<1>>, IF PrintT(<<"t", x>>) THEN <<1>> ELSE <<1>>) = <<9>>
====
