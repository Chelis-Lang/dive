---- MODULE C_root_3 ----
EXTENDS Integers
(* --algorithm A {variables x1=0; {skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "a1fb693a" /\ chksum(tla) = "8cb3a856")
VARIABLES pc, x1

vars == << pc, x1 >>

Init == (* Global variables *)
        /\ x1 = 0
        /\ pc = "L1"

L1 == /\ pc = "L1"
      /\ TRUE
      /\ pc' = "Done"
      /\ x1' = x1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
