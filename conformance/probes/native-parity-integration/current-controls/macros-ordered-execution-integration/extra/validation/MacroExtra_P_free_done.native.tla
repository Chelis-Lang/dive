---- MODULE MacroExtra_P_free_done ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro;macro Unused() begin goto Done; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
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

====
