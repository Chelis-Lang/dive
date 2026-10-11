---- MODULE P_root_4 ----
EXTENDS Integers
(* --algorithm A variables x1=0; begin skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "2953946e" /\ chksum(tla) = "59aff04c")
VARIABLES pc, x1

vars == << pc, x1 >>

Init == (* Global variables *)
        /\ x1 = 0
        /\ pc = "defaultInitValue1"

defaultInitValue1 == /\ pc = "defaultInitValue1"
                     /\ TRUE
                     /\ pc' = "Done"
                     /\ x1' = x1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == defaultInitValue1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
