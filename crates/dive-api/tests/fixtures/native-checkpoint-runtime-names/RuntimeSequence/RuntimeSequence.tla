---- MODULE RuntimeSequence ----
EXTENDS Naturals,TLC,Sequences
VARIABLE n,s
Init == n=0 /\ s="zero"
Next == n<4 /\ n'=n+1 /\ s'=SubSeq(Tail("value" \o ToString(n+1)),1,4)
Inv == n<3
View == <<n,s>>
====
