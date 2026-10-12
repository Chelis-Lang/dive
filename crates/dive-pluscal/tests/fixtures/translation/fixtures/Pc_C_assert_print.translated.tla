---- MODULE Pc_C_assert_print ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { variable x = 0, y; {
 assert TRUE; print <<1, 2>>;
} }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "472c885a" /\ chksum(tla) = "ae57a839")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ Assert(TRUE, "Failure of assertion at line 5, column 2.")
         /\ PrintT(<<1, 2>>)
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
