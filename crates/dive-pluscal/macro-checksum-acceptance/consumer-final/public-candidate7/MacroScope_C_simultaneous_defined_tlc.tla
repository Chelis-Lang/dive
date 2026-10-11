---- MODULE MacroScope_C_simultaneous_defined ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(a,b) { x:=<<a,b>>; }; { L: Put(b,3); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "cf113bc1" /\ chksum(tla) = "f27023e0")
VARIABLES pc, x, b

vars == << pc, x, b >>

Init == /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = << b , 3 >>
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
