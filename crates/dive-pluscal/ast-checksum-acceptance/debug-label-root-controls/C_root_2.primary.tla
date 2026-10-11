---- MODULE C_root_2 ----
EXTENDS Integers
(* --algorithm A {variables x1=0; {skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "2ab6a823" /\ chksum(tla) = "f667858f")
\* Label x1 at line 3 col 36 changed to x1_
VARIABLES pc, x1

vars == << pc, x1 >>

Init == (* Global variables *)
        /\ x1 = 0
        /\ pc = "x1_"

x1_ == /\ pc = "x1_"
       /\ TRUE
       /\ pc' = "Done"
       /\ x1' = x1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == x1_
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
