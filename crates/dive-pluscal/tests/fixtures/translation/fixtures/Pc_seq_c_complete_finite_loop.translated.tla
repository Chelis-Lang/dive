---- MODULE Pc_seq_c_complete_finite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: while (x<2) {x:=x+1;}; E: skip; }}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "dc75e8a8" /\ chksum(tla) = "d2d6a094")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ x' = x+1
                /\ pc' = "L"
           ELSE /\ pc' = "E"
                /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
