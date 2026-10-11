---- MODULE Macro_C_nested_rhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := 2*a + a; }; { L: Put(x, 1+2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "6375d1cd" /\ chksum(tla) = "f834f9a4")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 2 * ( 1 + 2 ) + ( 1 + 2 )
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
