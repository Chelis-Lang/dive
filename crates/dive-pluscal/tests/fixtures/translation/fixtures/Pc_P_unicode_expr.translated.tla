---- MODULE Pc_P_unicode_expr ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A variable x ∈ {1, 2}; begin assert ∃ a ∈ {1, 2} : a = a; x := ⟨1, 2⟩; end algorithm

*)
\* BEGIN TRANSLATION (chksum(pcal) = "bb7e7057" /\ chksum(tla) = "c3686b7f")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x \in {1, 2}
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ Assert(∃ a ∈ {1, 2} : a = a, 
                   "Failure of assertion at line 4, column 42.")
         /\ x' = ⟨1, 2⟩
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
