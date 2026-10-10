---- MODULE Pc_seq_c_current_assert ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: x:=1; assert x=0; E: skip; }}
*)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ Assert(x' = 0, "Failure of assertion at line 5, column 27.")
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
