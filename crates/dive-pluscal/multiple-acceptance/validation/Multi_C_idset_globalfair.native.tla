---- MODULE Multi_C_idset_globalfair ----
EXTENDS Naturals,TLC
(* --fair algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; E: g:=x; }  process (Q \in{2,3}) variable y=0; { M: y:=2; F: g:=y; } } *)

VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup ({ 2 , 3 })

Init == /\ g = 0
        /\ x = 0
        /\ y = [self \in { 2 , 3 } |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self \in { 2 , 3 } -> "M"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << g, y >>

E == /\ pc[1] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

M(self) == /\ pc[self] = "M"
           /\ y' = [y EXCEPT ![self] = 2]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << g, x >>

F(self) == /\ pc[self] = "F"
           /\ g' = y[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y >>

Q(self) == M(self) \/ F(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ (\E self \in { 2 , 3 }: Q(self)) \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
