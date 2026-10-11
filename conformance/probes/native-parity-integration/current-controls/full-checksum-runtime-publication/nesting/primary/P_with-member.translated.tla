---- MODULE P_with_member ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:with a \in {0,1},b \in {1,2} do x:=a+b; end with; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a417d822" /\ chksum(tla) = "9e787076")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ \E a \in {0,1}:
          \E b \in {1,2}:
            x' = a+b
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
