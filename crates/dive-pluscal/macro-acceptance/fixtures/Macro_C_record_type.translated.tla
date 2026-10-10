---- MODULE Macro_C_record_type ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(a) { x := [a: {a}]; }; { L: Put(2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "c3a6d45e" /\ chksum(tla) = "d68dd302")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [a: {2}]
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
