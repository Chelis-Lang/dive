---- MODULE Marker_P_unfair_first_next_line_crlf ----
EXTENDS Integers
(* --algorithm A variable x = 0; begin L: x := 1; end algorithm;
--fair algorithm B variable x = 0; begin L: x := 2; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "dbedd2a2" /\ chksum(tla) = "a41501b")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
