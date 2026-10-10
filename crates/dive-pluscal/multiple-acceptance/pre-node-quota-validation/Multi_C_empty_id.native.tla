---- MODULE Multi_C_empty_id ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: x:=1; E: g:=x; }  process (Q =1) variable y=0; { M: y:=2; F: g:=y; } } *)

VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == ({ }) \cup {1}

Init == /\ g = 0
        /\ x = [self \in { } |-> 0]
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self \in { } -> "L" [] self = 1 -> "M"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << g, y >>

E(self) == /\ pc[self] = "E"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y >>

P(self) == L(self) \/ E(self)

M == /\ pc[1] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc[1] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q \/ (\E self \in { }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
