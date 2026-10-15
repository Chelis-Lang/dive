---- MODULE ChoosePrint ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; begin L: print CHOOSE k \in {1}: TRUE; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "56ba8554" /\ chksum(tla) = "c86a7e5b")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ PrintT(CHOOSE k \in {1}: TRUE)
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
