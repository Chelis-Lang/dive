---- MODULE MacroExtra_P_free_global ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := y+a; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1b3d21f8" /\ chksum(tla) = "87bbb3be")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = y + 1
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
