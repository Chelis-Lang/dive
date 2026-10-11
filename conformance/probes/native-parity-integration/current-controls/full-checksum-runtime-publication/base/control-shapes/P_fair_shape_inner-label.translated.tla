---- MODULE P_fair_shape_inner_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0; begin L: if x=0 then B:x:=1; else skip; end if; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "97cdd141")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ pc' = "B"
           ELSE /\ TRUE
                /\ pc' = "Done"
     /\ x' = x

B == /\ pc = "B"
     /\ x' = 1
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ B
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
