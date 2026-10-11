---- MODULE Rewrite_C ----
\* λ🙂 preface
EXTENDS Integers
(* --algorithm A { variable x=0; { L:x:=1; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "40fe0264" /\ chksum(tla) = "f56fbb5c")
VARIABLES pc, x

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
\* preserved footer λ🙂
====
