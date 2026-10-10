---- MODULE Multi_P_mixed ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: x:=1; E: g:=x; end process;  process Q ="q" variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)

VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup {"q"}

Init == /\ g = 0
        /\ x = 0
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = "q" -> "M"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << g, y >>

E == /\ pc[1] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

M == /\ pc["q"] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT !["q"] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc["q"] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT !["q"] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
