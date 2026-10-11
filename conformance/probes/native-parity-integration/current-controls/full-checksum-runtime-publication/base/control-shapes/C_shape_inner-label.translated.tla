---- MODULE C_shape_inner_label ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: if(x=0){B:x:=1;}else{skip;} } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "93b01db0" /\ chksum(tla) = "222f14d0")
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

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
