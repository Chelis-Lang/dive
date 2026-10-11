---- MODULE C_token_spaced ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x = <<0,1>>; { L:x[1] := 1; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "69f7eb03" /\ chksum(tla) = "2445457f")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = <<0,1>>
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [x EXCEPT ![1] = 1]
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
