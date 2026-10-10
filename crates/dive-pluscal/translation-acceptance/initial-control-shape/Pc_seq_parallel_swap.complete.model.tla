---- MODULE Pc_seq_parallel_swap ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=1, y=2; begin L: x := y || y := x; E: skip; end algorithm;
*)

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 1
        /\ y = 2
        /\ pc = "L"

L == /\ pc = "L"
     /\ /\ x' = y
        /\ y' = x
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
