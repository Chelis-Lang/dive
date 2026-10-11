---- MODULE P_backslash ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x={0};
begin L: x := x \cup {1}; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "fb90761a" /\ chksum(tla) = "643e4f17")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = {0}
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (x \cup {1})
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
