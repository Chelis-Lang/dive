---- MODULE Pc_seq_empty_variables ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
begin L: skip; E: skip; end algorithm;
*)

VARIABLE pc

vars == << pc >>

Init == /\ pc = "L"

L == /\ pc = "L"
     /\ TRUE
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
