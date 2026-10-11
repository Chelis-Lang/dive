---- MODULE C_macro_empty_parameters ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; macro M() {x:=1+
 2;} { L:M(); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "7102c225" /\ chksum(tla) = "629a31b6")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                                   1+
             2
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
