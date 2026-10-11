---- MODULE C_fixed_elsif_in_while ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:while(x<2){if(x=0){x:=1;}else{if(x=1){x:=2;}}}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "d1e70801" /\ chksum(tla) = "47195f22")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ IF x=0
                      THEN /\ x' = 1
                      ELSE /\ IF x=1
                                 THEN /\ x' = 2
                                 ELSE /\ TRUE
                                      /\ x' = x
                /\ pc' = "L"
           ELSE /\ pc' = "E"
                /\ x' = x
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
