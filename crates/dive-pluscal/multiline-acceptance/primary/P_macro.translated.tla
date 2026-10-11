---- MODULE P_macro ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; macro M() begin x:=1+
 2; end macro; begin L:M(); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7102c225" /\ chksum(tla) = "941bf8c9")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                                       1+
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
