---- MODULE Pc_seq_quoted_done ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: x:=1; goto "Done"; end algorithm;
*)

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

====
