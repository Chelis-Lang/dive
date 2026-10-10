---- MODULE Pc_seq_elsif ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: if x=0 then x:=1; elsif x=1 then x:=2; else x:=3; end if; E: skip; end algorithm;
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
