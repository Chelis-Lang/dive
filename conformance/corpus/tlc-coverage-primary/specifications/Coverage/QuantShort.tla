---- MODULE QuantShort ----
EXTENDS Naturals
VARIABLE x
Some(S) == \E i \in S: i>1
Init == /\ x=0 /\ Some({1,2,3})
Next == x'=x
====
