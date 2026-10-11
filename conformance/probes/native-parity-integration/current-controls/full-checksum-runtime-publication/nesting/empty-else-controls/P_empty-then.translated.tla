---- MODULE P_empty_then ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if x=0 then else x:=1; end if; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4b0291" /\ chksum(tla) = "bbd38e0d")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = x
           ELSE /\ x' = 1
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
