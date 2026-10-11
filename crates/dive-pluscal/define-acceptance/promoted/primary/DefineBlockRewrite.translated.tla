---- MODULE R ----
EXTENDS Integers
(* --algorithm A variable x=0; define F == 1 end define; begin L:x:=1; end algorithm; *) \* tail
\* BEGIN TRANSLATION (chksum(pcal) = "2b2547fe" /\ chksum(tla) = "1752912d")
VARIABLES pc, x

(* define statement *)
F == 1


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
