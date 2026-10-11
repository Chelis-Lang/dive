---- MODULE P_fair_inner_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0;
begin L: while x<2 do B:x:=x+1; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "c2e417f0")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ pc' = "B"
           ELSE /\ pc' = "Done"
     /\ x' = x

B == /\ pc = "B"
     /\ x' = x+1
     /\ pc' = "L"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ B
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
