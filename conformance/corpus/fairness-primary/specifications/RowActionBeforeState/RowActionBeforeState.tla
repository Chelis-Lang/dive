---- MODULE RowActionBeforeState ----
EXTENDS Naturals
VARIABLES s, x
Init == s = 0 /\ x = 0
Pick == s' = 1 /\ x' = CHOOSE v \in {1, 2} : v > 5
Next == UNCHANGED <<s,x>>
Spec == Init /\ [][Next]_<<s, x>> /\ WF_<<s, x>>(Pick)
BadState == (CHOOSE v \in {1,2}: v>9+x)=0
Live == <>[]BadState
====
