---- MODULE C_token_record ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x = [a |-> 0]; { L:x.a:=1; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "699469b4" /\ chksum(tla) = "b8c075a4")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = [a |-> 0]
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [x EXCEPT !.a = 1]
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
====
