---- MODULE SeqReplaceOperatorEffect ----
EXTENDS Naturals, Sequences, SequencesExt, TLC
VARIABLE x
Init == x = 0
Next == x < 1 /\ x' = x + 1
Replace(F(_), a) == ReplaceFirstSubSeq(F(a), <<1>>, <<1>>)
R(a) == IF PrintT(<<"r", a>>) THEN <<9>> ELSE <<9>>
Inv == Replace(R, x) = <<9>>
====
