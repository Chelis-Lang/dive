---- MODULE R ----
EXTENDS Integers
(* --algorithm A variable x=0; define F == 1 end define; begin L:x:=1; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "c3b157cb" /\ chksum(tla) = "e3faf4f0")
VARIABLES pc, x

(* define statement *)
F == 1


vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
\* footer
====
