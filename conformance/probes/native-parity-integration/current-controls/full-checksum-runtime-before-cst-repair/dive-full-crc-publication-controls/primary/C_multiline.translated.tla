---- MODULE C_multiline ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x=0; { L: x := (1 +
 2); } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "42b7fb93" /\ chksum(tla) = "95b72bef")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                            (1 +
             2)
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
