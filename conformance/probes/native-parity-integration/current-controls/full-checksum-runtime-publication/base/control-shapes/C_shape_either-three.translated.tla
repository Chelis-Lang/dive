---- MODULE C_shape_either_three ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: either {x:=1;} or {x:=2;} or {skip;} } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "6ba02066" /\ chksum(tla) = "c943c4")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ \/ /\ x' = 1
        \/ /\ x' = 2
        \/ /\ TRUE
           /\ x' = x
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
