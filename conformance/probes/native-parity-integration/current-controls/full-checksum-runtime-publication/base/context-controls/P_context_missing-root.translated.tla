---- MODULE P_context_missing_root ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "e0cfef38" /\ chksum(tla) = "108fd2f4")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ TRUE
         /\ pc' = "Done"
         /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
