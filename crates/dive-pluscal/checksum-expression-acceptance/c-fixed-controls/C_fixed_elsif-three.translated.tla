---- MODULE C_fixed_elsif_three ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if(x=0){x:=1;}else{if(x=1){x:=2;}else{if(x=2){skip;}else{x:=3;}}}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "dbeb9189" /\ chksum(tla) = "c7d1a59b")
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
                      ELSE /\ IF x=2
                                 THEN /\ TRUE
                                      /\ x' = x
                                 ELSE /\ x' = 3
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
