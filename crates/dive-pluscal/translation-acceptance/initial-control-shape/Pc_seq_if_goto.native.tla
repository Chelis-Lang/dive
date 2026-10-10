---- MODULE Pc_seq_if_goto ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: if x=0 then goto E; else skip; end if; E: skip; end algorithm;
*)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x = 0
           THEN /\ pc' = "E"
           ELSE /\ TRUE
                /\ pc' = "E"
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
