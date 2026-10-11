---- MODULE MacroExtra_C_triple ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; };macro Outer(v,a) { Put(v,a+1); };macro Last(v,a) { Outer(v,a*2); }; { L: Last(x, 2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f7e21278" /\ chksum(tla) = "f95a5df3")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = ( 2 * 2 ) + 1
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
