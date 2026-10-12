---- MODULE P_case_true_one ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:x:=CASE TRUE -> 1 [] OTHER -> 0; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ecfe940e" /\ chksum(tla) = "383f9c37")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (CASE TRUE -> 1 [] OTHER -> 0)
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
