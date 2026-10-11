---- MODULE C_root_5 ----
EXTENDS Integers
(* --algorithm A {variables x1=0; {skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "17afa3c9" /\ chksum(tla) = "8b9abbf3")
VARIABLES pc, x1

vars == << pc, x1 >>

Init == (* Global variables *)
        /\ x1 = 0
        /\ pc = "Done1"

Done1 == /\ pc = "Done1"
         /\ TRUE
         /\ pc' = "Done"
         /\ x1' = x1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Done1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
