---- MODULE Pc_seq_c_complete_either ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variables x=0,y=0; { L: either {x:=1;} or {y:=2;}; E: skip; }}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "a7285744" /\ chksum(tla) = "e920649f")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
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

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
