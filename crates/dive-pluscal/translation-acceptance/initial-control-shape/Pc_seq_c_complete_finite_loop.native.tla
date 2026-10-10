---- MODULE Pc_seq_c_complete_finite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: while (x<2) {x:=x+1;}; E: skip; }}
*)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x < 2
           THEN /\ x' = x + 1
                /\ pc' = "L"
           ELSE /\ pc' = "E"
                /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
