---- MODULE CmSeqsViol ----
\* A violation whose counts depend on the order SetToSeqs enumerates in.
EXTENDS Naturals, SequencesExt
VARIABLE s, n
Init == s = <<>> /\ n = 0
Next == n < 2 /\ \E q \in SetToSeqs(1..4) : s' = s \o q /\ n' = n + 1
Inv == ~(n = 2 /\ s = <<4, 2, 3, 1, 3, 1, 4, 2>>)
====
