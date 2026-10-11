---- MODULE C_token_quoted_escape ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x="a\"b"; { L:x:="c\\d"; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "e31cce1e" /\ chksum(tla) = "ade05e7b")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = "a\"b"
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = "c\\d"
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
