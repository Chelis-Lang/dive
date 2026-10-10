---- MODULE Pc_C_unicode ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { variable x = 0, y; {
 x ≔ 1 ‖ y ≔ 2; assert ∀ a ∈ {1, 2} : a = a;
} }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "caf4ae79" /\ chksum(tla) = "3b97aeb1")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ /\ x' = 1
            /\ y' = 2
         /\ Assert(∀ a ∈ {1, 2} : a = a, 
                   "Failure of assertion at line 5, column 17.")
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
