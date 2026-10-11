---- MODULE Marker_P_fair_first_next_line_crlf ----
EXTENDS Integers
(* --fair algorithm A variable x = 0; begin L: x := 1; end algorithm;
--algorithm B variable x = 0; begin L: x := 2; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "5408c87b")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
