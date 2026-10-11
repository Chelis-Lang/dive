---- MODULE C_fair_shape_elsif ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0; { L: if (x=0) { x:=1; } else { if (x=1) { x:=2; } else {skip;} } } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "7184801a")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = 1
           ELSE /\ IF x=1
                      THEN /\ x' = 2
                      ELSE /\ TRUE
                           /\ x' = x
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
