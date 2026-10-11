---- MODULE MacroScope_C_qualified_actual ----
EXTENDS Integers, TLC
---- MODULE Helper ----
a == 2
====
I == INSTANCE Helper
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:=a; }; { L: Put(x,I!a); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "5804d4a4" /\ chksum(tla) = "776fa98")
VARIABLES pc, x, b

vars == << pc, x, b >>

Init == /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = I ! a
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
