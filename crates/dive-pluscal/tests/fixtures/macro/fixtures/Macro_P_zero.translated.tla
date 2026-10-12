---- MODULE Macro_P_zero ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put() begin x:=1; end macro; begin L: Put(); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d06ecb2b" /\ chksum(tla) = "665e7a4b")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
