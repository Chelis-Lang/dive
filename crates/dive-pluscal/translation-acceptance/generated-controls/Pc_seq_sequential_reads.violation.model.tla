---- MODULE Pc_seq_sequential_reads ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=0, y=0; begin L: x := x+1; y := x; E: skip; end algorithm;
*)

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = x + 1
     /\ y' = x'
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == x = 0
====
