---- MODULE MacroExtra_P_triple ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro;macro Outer(v,a) begin Put(v,a+1); end macro;macro Last(v,a) begin Outer(v,a*2); end macro; begin L: Last(x, 2); E: skip; end algorithm; *)
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
