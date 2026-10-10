---- MODULE Pc_P_definitions ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A define D == INSTANCE M WITH a <- 1, b <- 2 end define; begin skip; end algorithm

*)
\* BEGIN TRANSLATION (chksum(pcal) = "e89da82e" /\ chksum(tla) = "d6f74e29")
VARIABLE pc

(* define statement *)
D == INSTANCE M WITH a <- 1, b <- 2


vars == << pc >>

Init == /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ TRUE
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
