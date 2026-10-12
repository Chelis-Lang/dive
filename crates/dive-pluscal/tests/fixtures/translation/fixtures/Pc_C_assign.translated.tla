---- MODULE Pc_C_assign ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { variable x = 0, y; {
 x := 1;
} }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "811ffa10" /\ chksum(tla) = "d82575fc")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = 1
         /\ pc' = "Done"
         /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
