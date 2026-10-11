---- MODULE C_junction_nested ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:await /\ (y=0 \/ y=1)
      /\ TRUE; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "b331926a" /\ chksum(tla) = "85bf5a27")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\                                          /\ (y=0 \/ y=1)
        /\ TRUE
     /\ pc' = "E"
     /\ UNCHANGED << x, y >>

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
