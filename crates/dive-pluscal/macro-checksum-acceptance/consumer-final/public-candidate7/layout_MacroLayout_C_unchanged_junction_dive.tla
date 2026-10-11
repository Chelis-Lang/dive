---- MODULE MacroLayout_C_unchanged_junction ----
EXTENDS Integers,TLC
(* --algorithm A {variables x=0; macro Put(v,a){v:=TRUE /\ FALSE;}; {L:Put(x,1); E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "1ee2ebea" /\ chksum(tla) = "f1dfdf4a")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (TRUE /\ FALSE)
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
