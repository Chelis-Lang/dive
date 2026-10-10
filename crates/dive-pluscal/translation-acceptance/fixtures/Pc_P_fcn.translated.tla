---- MODULE Pc_P_fcn ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A begin assert [a \in {1, 2} |-> a] = [a \in {1, 2} |-> a]; end algorithm

*)
\* BEGIN TRANSLATION (chksum(pcal) = "1547f86c" /\ chksum(tla) = "df011936")
VARIABLE pc

vars == << pc >>

Init == /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ Assert([a \in {1, 2} |-> a] = [a \in {1, 2} |-> a], 
                   "Failure of assertion at line 4, column 21.")
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
