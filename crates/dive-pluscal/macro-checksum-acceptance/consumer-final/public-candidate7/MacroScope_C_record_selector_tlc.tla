---- MODULE MacroScope_C_record_selector ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:= [a |-> a].a; }; { L: Put(x,2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "950bcaa6" /\ chksum(tla) = "7f77cc08")
VARIABLES pc, x, b

vars == << pc, x, b >>

Init == /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [ a |-> 2 ] . a
     /\ pc' = "E"
     /\ b' = b

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, b >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
