---- MODULE Pc_seq_c_current_parallel_swap ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variables x=1,y=2; { L: x:=y || y:=x; E: skip; }}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "389d6105" /\ chksum(tla) = "ced3e7cf")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 1
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

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
