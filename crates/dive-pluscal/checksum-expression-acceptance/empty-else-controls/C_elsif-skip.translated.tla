---- MODULE C_elsif_skip ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if(x=0){x:=1;}else{if(x=1){x:=2;}else{skip;}}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "e1d3fda3" /\ chksum(tla) = "c978f816")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = 1
           ELSE /\ IF x=1
                      THEN /\ x' = 2
                      ELSE /\ TRUE
                           /\ x' = x
     /\ pc' = "E"
     /\ y' = y

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
