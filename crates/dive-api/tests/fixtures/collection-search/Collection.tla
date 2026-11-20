---- MODULE Collection ----
EXTENDS Integers, SequencesExt, BagsExt, TLC
VARIABLE x
Init == x = 0
Next == x' = 1-x
S == <<0,1,0,1>>
Match(v) == v=x
Positions == /\ SelectInSubSeq(S,2,4,Match) = IF x=0 THEN 3 ELSE 2
             /\ SelectLastInSubSeq(S,2,4,Match) = IF x=0 THEN 3 ELSE 4
             /\ SelectLastInSubSeq(S,9,-1,Match)=0
             /\ SelectInSubSeq(S,2,2,LAMBDA e: e=x) = IF x=0 THEN 0 ELSE 2
Bad == SelectLastInSubSeq(S,2,4,Match) = 3
Bag == FoldBag(LAMBDA a,e: a*10+e, x, (3:>1) @@ (1:>2))
Asc == Bag = IF x=0 THEN 113 ELSE 1113
TlcOrder == Bag = IF x=0 THEN 311 ELSE 1311
Low == SelectInSubSeq(S,0,3,Match)=0
High == SelectLastInSubSeq(S,2,5,Match)=0
A == [kind |-> "entry", payload |-> x]
B == [kind |-> "config", payload |-> [server |-> x]]
AB == (A :> 1) (+) (B :> 2)
Dup == AB (+) (A :> 1)
BagCounts == /\ BagCardinality(AB) = 3
        /\ AB[A] = 1 /\ AB[B] = 2
        /\ Dup[A] = 2 /\ Dup[B] = 2
        /\ (Dup (-) (B :> 1))[B] = 1
        /\ BagCardinality(BagUnion({A :> 1, B :> 2})) = 3
BagFailure == x = 0 \/ AB[A] = 2
====
