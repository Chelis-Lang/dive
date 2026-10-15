---- MODULE NameCdot ----
EXTENDS Naturals, Sequences
VARIABLE s
Init == s = "baba"
First == IF Len(s)=4
         THEN s' = (CHOOSE t \in {SubSeq(s,1,1),SubSeq(s,2,2)} : TRUE)
         ELSE s' = s
Last == s' = s
Next == First \cdot Last
Spec == Init /\ [][Next]_s /\ WF_s(First)
Hold == <>(Len(s)=1)
Fail == []<>(Len(s)=4)
====
