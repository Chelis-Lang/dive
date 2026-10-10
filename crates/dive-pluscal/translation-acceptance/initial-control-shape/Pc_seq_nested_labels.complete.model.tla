---- MODULE Pc_seq_nested_labels ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while x<2 do B: x := x+1; end while; E: skip; end algorithm;
*)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x < 2
           THEN /\ pc' = "B"
           ELSE /\ pc' = "E"
     /\ x' = x

B == /\ pc = "B"
     /\ x' = x + 1
     /\ pc' = "L"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ B \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
