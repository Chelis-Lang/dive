---- MODULE R ----
EXTENDS Integers
(* --algorithm A variable x=0; macro M() begin skip; end macro; begin L:x:=1; end algorithm; *) \* tail
\* BEGIN TRANSLATION (chksum(pcal) = "dbedd2a2" /\ chksum(tla) = "a41501b")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
\* footer λ🙂
====
