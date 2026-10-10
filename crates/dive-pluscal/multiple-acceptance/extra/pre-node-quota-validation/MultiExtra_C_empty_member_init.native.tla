---- MODULE MultiExtra_C_empty_member_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x \in {0,1}; { L: x:=1; LE: g:=x; }  process (Q =3) variable y \in {0,1}; { M: y:=2; ME: g:=y; }  process (R \in{}) variable z \in {0,1}; { N: z:=3; NE: g:=z; } } *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ }) \cup {3} \cup ({ })

Init == /\ g = 0
        /\ x \in [{ } -> { 0 , 1 }]
        /\ y \in { 0 , 1 }
        /\ z \in [{ } -> { 0 , 1 }]
        /\ pc = [self \in ProcSet |-> CASE self \in { } -> "L" [] self = 3 -> "M" [] self \in { } -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ z' = [z EXCEPT ![self] = 3]
           /\ pc' = [pc EXCEPT ![self] = "NE"]
           /\ UNCHANGED << g, x, y >>

NE(self) == /\ pc[self] = "NE"
            /\ g' = z[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

R(self) == N(self) \/ NE(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q \/ (\E self \in { }: P(self)) \/ (\E self \in { }: R(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
