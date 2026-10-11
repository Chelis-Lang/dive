---- MODULE P_layout_tab_prefix ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:	await /\ TRUE
	/\ y=0; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "26a5f4fe" /\ chksum(tla) = "eae2ae5e")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\                                               /\ TRUE
        /\ y=0
     /\ pc' = "E"
     /\ UNCHANGED << x, y >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
