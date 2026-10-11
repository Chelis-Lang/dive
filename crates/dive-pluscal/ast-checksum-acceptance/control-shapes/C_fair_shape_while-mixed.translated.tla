---- MODULE C_fair_shape_while_mixed ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0; { L: while (x<2) {x:=x+1; B:skip;} } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "4ed22146")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ x' = x+1
                /\ pc' = "B"
           ELSE /\ pc' = "Done"
                /\ x' = x

B == /\ pc = "B"
     /\ TRUE
     /\ pc' = "L"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ B
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
