---- MODULE Marker_C_unfair_first_same_line_lf ----
EXTENDS Integers
(* --algorithm A { variable x = 0; { L: x := 1; } } --fair algorithm B { variable x = 0; { L: x := 2; } } *)
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
