---- MODULE Case ----
EXTENDS Naturals, TLC

(* --algorithm A variables x = 0; begin A: x := 1; B: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d1c14d58" /\ chksum(tla) = "f2b11a0b")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "A"

A == /\ pc = "A"
     /\ x' = 1
     /\ pc' = "B"

B == /\ pc = "B"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == A \/ B
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
