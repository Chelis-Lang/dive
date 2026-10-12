---- MODULE Macro_C_field_rhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(a) { x := [a |-> a].a; }; { L: Put(2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "834ccbde" /\ chksum(tla) = "de8eac6c")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [a |-> 2].a
     /\ pc' = "E"
     /\ y' = y

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
