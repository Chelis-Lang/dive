---- MODULE CmSubSeqsViol ----
\* A violation reached through AllSubSeqs, whose counts depend on the order
\* the set is enumerated in.
EXTENDS Naturals, SequencesExt
VARIABLE s, n
Init == s = <<>> /\ n = 0
Next == n < 2 /\ \E q \in AllSubSeqs(<<1, 2, 3, 4>>) : s' = s \o q /\ n' = n + 1
Inv == ~(n = 2 /\ s = <<2, 4, 1, 3>>)
====
