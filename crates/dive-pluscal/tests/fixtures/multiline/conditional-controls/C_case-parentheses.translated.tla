---- MODULE C_case_parentheses ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=(CASE
  TRUE -> 1 [] OTHER -> 0); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "ebdcaf35" /\ chksum(tla) = "5ecd8dca")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                           (CASE
             TRUE -> 1 [] OTHER -> 0)
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
