---- MODULE MacroScope_P_simultaneous_defined ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(a,b) begin x:=<<a,b>>; end macro; begin L: Put(b,3); E: skip; end algorithm; *)

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


Good == pc # "E"
====
