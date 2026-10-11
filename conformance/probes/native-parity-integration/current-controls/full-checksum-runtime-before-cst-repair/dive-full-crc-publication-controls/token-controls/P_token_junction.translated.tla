---- MODULE P_token_junction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L:await /\ TRUE /\ x=0; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4f27fc36" /\ chksum(tla) = "42500b3d")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ /\ TRUE /\ x=0
     /\ pc' = "E"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
