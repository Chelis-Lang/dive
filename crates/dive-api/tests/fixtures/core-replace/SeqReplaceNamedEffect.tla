---- MODULE SeqReplaceNamedEffect ----
EXTENDS Naturals, Sequences, SequencesExt, TLC
VARIABLE x
Init == x = 0
Next == x < 1 /\ x' = x + 1
R(a) == IF PrintT(<<"r", a>>) THEN <<9>> ELSE <<9>>
Inv == ReplaceFirstSubSeq(R(x), <<1>>, <<1>>) = <<9>>
====
