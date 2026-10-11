---- MODULE C_shape_with_member ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: with (a \in {0,1}) {x:=a;} } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f6a8437" /\ chksum(tla) = "6e9dfe27")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ \E a \in {0,1}:
          x' = a
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
