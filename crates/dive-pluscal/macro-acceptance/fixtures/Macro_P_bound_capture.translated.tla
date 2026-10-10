---- MODULE Macro_P_bound_capture ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(a) begin x := {a \in 1..2 : a>0}; end macro; begin L: Put(2); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "eec928b1" /\ chksum(tla) = "1adf49f9")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = {2 \in 1..2 : 2>0}
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
