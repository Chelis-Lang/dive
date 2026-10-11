---- MODULE P_fair_shape_nested_if ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0; begin L: if x=0 then if x=1 then x:=2; else x:=1; end if; else skip; end if; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "608583df")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ IF x=1
                      THEN /\ x' = 2
                      ELSE /\ x' = 1
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
