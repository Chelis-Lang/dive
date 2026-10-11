---- MODULE C_token_empty ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x = <<>>; { L:x:=<<1>>; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "cea5e583" /\ chksum(tla) = "1dff35e3")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = <<>>
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = <<1>>
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
