---- MODULE Pc_seq_c_complete_either ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variables x=0,y=0; { L: either {x:=1;} or {y:=2;}; E: skip; }}
*)

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ \/ /\ x' = 1
           /\ y' = y
        \/ /\ y' = 2
           /\ x' = x
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
