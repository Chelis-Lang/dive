---- MODULE NameCdot ----
EXTENDS Naturals, Sequences
VARIABLE s
Init == s = "baba"
First == IF Len(s)=4
         THEN s' = (CHOOSE t \in {SubSeq(s,1,2),SubSeq(s,2,3)} : TRUE)
         ELSE s' = s
Last == s' = s
Next == First \cdot Last
Fair == Len(s)=4 /\ s'= ""
Spec == Init /\ [][Next]_s /\ WF_s(Fair)
Hold == <>(Len(s)=2)
Fail == []<>(Len(s)=4)
====
