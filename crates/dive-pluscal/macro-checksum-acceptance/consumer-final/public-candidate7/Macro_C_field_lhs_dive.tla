---- MODULE Macro_C_field_lhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=[a |-> 0], y=0; macro Put(v, a) { v := a; }; { L: Put(x.a,2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "94ac6685" /\ chksum(tla) = "78072020")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = [ a |-> 0 ]
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [x EXCEPT !. a = 2]
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
