---- MODULE C_case_declaration ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=CASE TRUE -> 1
  [] OTHER -> 0,y=0; { L:skip; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "d7282817" /\ chksum(tla) = "ea601e1e")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = (                            CASE TRUE -> 1
                [] OTHER -> 0)
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
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
