---- MODULE Pc_seq_branch_unchanged ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=0, y=0; begin L: if TRUE then x:=1; else y:=2; end if; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "8844f761" /\ chksum(tla) = "dde065a1")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF TRUE
           THEN /\ x' = 1
                /\ y' = y
           ELSE /\ y' = 2
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
