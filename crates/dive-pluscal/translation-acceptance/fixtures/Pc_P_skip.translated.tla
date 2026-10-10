---- MODULE Pc_P_skip ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A variable x = 0, y; begin
 skip;
end algorithm

*)
\* BEGIN TRANSLATION (chksum(pcal) = "5dcdc6a" /\ chksum(tla) = "4175ef74")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ TRUE
         /\ pc' = "Done"
         /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
