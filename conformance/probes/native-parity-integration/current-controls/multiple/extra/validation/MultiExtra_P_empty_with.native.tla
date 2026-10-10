---- MODULE MultiExtra_P_empty_with ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{} variable x=0; begin L: with b0 \in {1,2} do x:=b0; end with; LE: g:=x; end process;  process Q =3 variable y=0; begin M: with b1 \in {1,2} do y:=b1; end with; ME: g:=y; end process;  process R \in{} variable z=0; begin N: with b2 \in {1,2} do z:=b2; end with; NE: g:=z; end process; end algorithm; *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ }) \cup {3} \cup ({ })

Init == /\ g = 0
        /\ x = [self \in { } |-> 0]
        /\ y = 0
        /\ z = [self \in { } |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in { } -> "L" [] self = 3 -> "M" [] self \in { } -> "N"]

L(self) == /\ pc[self] = "L"
           /\ \E b0 \in { 1 , 2 }:
                 x' = [x EXCEPT ![self] = b0]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ \E b1 \in { 1 , 2 }:
           y' = b1
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ \E b2 \in { 1 , 2 }:
                 z' = [z EXCEPT ![self] = b2]
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
