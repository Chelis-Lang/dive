---- MODULE Macro_P_repeated ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; begin L: Put(x,1); E: Put(y,2); F: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "43385af1" /\ chksum(tla) = "5a41777d")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ y' = 2
     /\ pc' = "F"
     /\ x' = x

F == /\ pc = "F"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ F \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
