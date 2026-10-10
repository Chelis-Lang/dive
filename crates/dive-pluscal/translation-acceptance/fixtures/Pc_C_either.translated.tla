---- MODULE Pc_C_either ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { variable x = 0, y; {
 either x := 1; or x := 2; or skip;
} }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "de9c82e6" /\ chksum(tla) = "c48ade5a")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ \/ /\ x' = 1
            \/ /\ x' = 2
            \/ /\ TRUE
               /\ x' = x
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
