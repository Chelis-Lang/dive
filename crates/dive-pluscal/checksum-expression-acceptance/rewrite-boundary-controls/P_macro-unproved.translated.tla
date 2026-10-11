---- MODULE R ----
EXTENDS Integers
(* --algorithm A variable x=0; macro M() begin skip; end macro; begin L:x:=1; end algorithm; *) \* tail
\* BEGIN TRANSLATION (chksum(pcal) = "dbedd2a2" /\ chksum(tla) = "d14ef4d6")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
\* footer λ🙂
====
