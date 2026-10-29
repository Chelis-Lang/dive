---- MODULE SequenceErrorFirst ----
EXTENDS Integers, Sequences
VARIABLE s
Init == s = <<1>>
Next == /\ s' = <<2,3>>
        /\ (SelectSeq(Len(s), LAMBDA n : 1 \div 0 > n))' = <<>>
====
