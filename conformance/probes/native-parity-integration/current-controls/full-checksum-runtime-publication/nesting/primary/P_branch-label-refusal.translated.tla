---- MODULE P_branch_label_refusal ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if x=0 then B:x:=1; else skip; end if; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4cc2e500" /\ chksum(tla) = "826f450")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ pc' = "B"
           ELSE /\ TRUE
                /\ pc' = "E"
     /\ UNCHANGED << x, y >>

B == /\ pc = "B"
     /\ x' = 1
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ B \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
