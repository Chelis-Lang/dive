---- MODULE Macro_P_field_rhs ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(a) begin x := [a |-> a].a; end macro; begin L: Put(2); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "834ccbde" /\ chksum(tla) = "65eb7475")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [ a |-> 2 ] . a
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
