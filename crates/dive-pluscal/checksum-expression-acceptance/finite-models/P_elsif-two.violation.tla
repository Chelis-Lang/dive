---- MODULE P_elsif_two ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if x=0 then x:=1; elsif x=1 then x:=2; else skip; end if; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "e1d3fda3" /\ chksum(tla) = "c31493ba")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x = 0
           THEN /\ x' = 1
           ELSE /\ IF x = 1
                      THEN /\ x' = 2
                      ELSE /\ TRUE
                           /\ x' = x
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 

Good == pc # "E"
====
