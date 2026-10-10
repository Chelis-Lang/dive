---- MODULE MultiExtra_C_mixed_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variables x=0, xd=x+1; { L: x:=1; LE: g:=x; }  process (Q =3) variables y=0, yd=y+1; { M: y:=2; ME: g:=y; }  process (R \in{4,5}) variables z=0, zd=z+1; { N: z:=3; NE: g:=z; } } *)

VARIABLES pc, g, x, xd, y, yd, z, zd

vars == << pc, g, x, xd, y, yd, z, zd >>

ProcSet == ({ 1 , 2 }) \cup {3} \cup ({ 4 , 5 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]
        /\ xd = [self \in { 1 , 2 } |-> x[self] + 1]
        /\ y = 0
        /\ yd = y + 1
        /\ z = [self \in { 4 , 5 } |-> 0]
        /\ zd = [self \in { 4 , 5 } |-> z[self] + 1]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self = 3 -> "M" [] self \in { 4 , 5 } -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, xd, y, yd, z, zd >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, xd, y, yd, z, zd >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, xd, yd, z, zd >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, xd, y, yd, z, zd >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ z' = [z EXCEPT ![self] = 3]
           /\ pc' = [pc EXCEPT ![self] = "NE"]
           /\ UNCHANGED << g, x, xd, y, yd, zd >>

NE(self) == /\ pc[self] = "NE"
            /\ g' = z[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, xd, y, yd, z, zd >>

R(self) == N(self) \/ NE(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q \/ (\E self \in { 1 , 2 }: P(self)) \/ (\E self \in { 4 , 5 }: R(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
