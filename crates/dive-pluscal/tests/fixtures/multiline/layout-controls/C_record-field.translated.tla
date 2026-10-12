---- MODULE C_record_field ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:	(* 😀 outside strings *) x:=1; y:=[x |-> x,
	z |-> x].x; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "fa7bea90" /\ chksum(tla) = "beee52ad")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ y' =                                                                           [x |-> x',
             z |-> x'].x
     /\ pc' = "E"

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
