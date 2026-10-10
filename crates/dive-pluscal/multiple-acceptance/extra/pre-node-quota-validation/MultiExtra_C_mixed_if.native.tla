---- MODULE MultiExtra_C_mixed_if ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: if (x=0) { x:=1; } else { x:=0; }; LE: g:=x; }  process (Q =3) variable y=0; { M: if (y=0) { y:=2; } else { y:=0; }; ME: g:=y; }  process (R \in{4,5}) variable z=0; { N: if (z=0) { z:=3; } else { z:=0; }; NE: g:=z; } } *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ 1 , 2 }) \cup {3} \cup ({ 4 , 5 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]
        /\ y = 0
        /\ z = [self \in { 4 , 5 } |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self = 3 -> "M" [] self \in { 4 , 5 } -> "N"]

L(self) == /\ pc[self] = "L"
           /\ IF x[self] = 0
                 THEN /\ x' = [x EXCEPT ![self] = 1]
                 ELSE /\ x' = [x EXCEPT ![self] = 0]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ IF y = 0
           THEN /\ y' = 2
           ELSE /\ y' = 0
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ IF z[self] = 0
                 THEN /\ z' = [z EXCEPT ![self] = 3]
                 ELSE /\ z' = [z EXCEPT ![self] = 0]
           /\ pc' = [pc EXCEPT ![self] = "NE"]
           /\ UNCHANGED << g, x, y >>

NE(self) == /\ pc[self] = "NE"
            /\ g' = z[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

R(self) == N(self) \/ NE(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q \/ (\E self \in { 1 , 2 }: P(self)) \/ (\E self \in { 4 , 5 }: R(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
