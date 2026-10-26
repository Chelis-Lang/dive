---------------- MODULE StartupStoredRead ----------------
EXTENDS Naturals, TLC
CONSTANT C
Draw == IF PrintT("DRAW") THEN RandomElement({0, 1}) ELSE 0
VARIABLE x
Init == x = 0
Next == /\ x' = 1-x
        /\ Draw \in {0, 1}
Spec == Init /\ [][Next]_x /\ WF_x(Next)
DirectLive == []<>(x = Draw)
RedirectLive == []<>(x = C)
==========================================================
