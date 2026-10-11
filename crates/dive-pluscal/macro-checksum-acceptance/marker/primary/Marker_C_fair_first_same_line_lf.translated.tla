---- MODULE Marker_C_fair_first_same_line_lf ----
EXTENDS Integers
(* --fair algorithm A { variable x = 0; { L: x := 1; } } --algorithm B { variable x = 0; { L: x := 2; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "7f9034b6" /\ chksum(tla) = "3a9ebb2e")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 2
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
