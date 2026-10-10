---- MODULE Proc_P_id_parallel_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0,y=0; begin L: x:=y+1 || y:=x+1; E: g:=x+y; end process; end algorithm; *)

VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1}

Init == /\ g = 0
        /\ x = 0
        /\ y = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ /\ x' = y + 1
        /\ y' = x + 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ g' = g

E == /\ pc[1] = "E"
     /\ g' = x + y
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
