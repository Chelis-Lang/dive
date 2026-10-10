---- MODULE Pc_P_selectors ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A variable x = 0, y; begin
 x[1].foo := 2;
end algorithm

*)
\* BEGIN TRANSLATION (chksum(pcal) = "86e281e8" /\ chksum(tla) = "ad8cad7c")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = [x EXCEPT ![1].foo = 2]
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
