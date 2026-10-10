---- MODULE Pc_seq_c_complete_elsif ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: if (x=0) {x:=1;} else if (x=1) {x:=2;} else {x:=3;}; E: skip; }}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "536f903e" /\ chksum(tla) = "23b80846")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = 1
           ELSE /\ IF x=1
                      THEN /\ x' = 2
                      ELSE /\ x' = 3
     /\ pc' = "E"

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
