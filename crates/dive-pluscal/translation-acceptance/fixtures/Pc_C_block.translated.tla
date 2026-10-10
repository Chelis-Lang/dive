---- MODULE Pc_C_block ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { { L: { skip; print 1; }; skip; } }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "bf0265ce" /\ chksum(tla) = "77254b01")
VARIABLE pc

vars == << pc >>

Init == /\ pc = "L"

L == /\ pc = "L"
     /\ TRUE
     /\ PrintT(1)
     /\ TRUE
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
