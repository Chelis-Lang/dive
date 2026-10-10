---- MODULE MacroScope_P_with_shadow ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin with a=3 do v:=a; end with; end macro; begin L: Put(x,2); E: skip; end algorithm; *)

VARIABLES pc, x, b

vars == << pc, x, b >>

Init == /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 3 IN
           x' = 2
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

====
