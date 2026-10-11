---- MODULE P_root_0 ----
EXTENDS Integers
(* --algorithm A variables x1=0; begin skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4f230720" /\ chksum(tla) = "9091a634")
VARIABLES pc, x1

vars == << pc, x1 >>

Init == (* Global variables *)
        /\ x1 = 0
        /\ pc = "Root_1"

Root_1 == /\ pc = "Root_1"
          /\ TRUE
          /\ pc' = "Done"
          /\ x1' = x1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Root_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
