---- MODULE PredicateError ----
EXTENDS Integers, Sequences
VARIABLE s
Init == s = <<1>>
Next == /\ s' = <<2,3>>
        /\ (SelectSeq(s, LAMBDA n : 1 \div (n - 3) > 0))' = <<>>
====
