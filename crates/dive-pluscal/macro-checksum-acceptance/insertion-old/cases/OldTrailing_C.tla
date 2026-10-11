---- MODULE OldTrailing_C ----
EXTENDS Integers
(* --algorithm A { variable x=0; macro Set(v,a) { v:=a; }; { L:Set(x,1+2); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "4f4af27b" /\ chksum(tla) = "7f927289")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1+2
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
Inv == x = 3
====
