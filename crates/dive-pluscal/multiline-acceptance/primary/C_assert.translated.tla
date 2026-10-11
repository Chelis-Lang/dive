---- MODULE C_assert ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:assert y=0
  = TRUE; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "277da8d3" /\ chksum(tla) = "c3e58174")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Assert(                                              y=0
               = TRUE, "Failure of assertion at line 3, column 42.")
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
