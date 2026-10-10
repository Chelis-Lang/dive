---- MODULE MultiExtra_P_mixed_modifiers ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; fair process P \in{1,2} variable x=0; begin L: + x:=1; LE: - g:=x; end process; fair process Q =3 variable y=0; begin M: + y:=2; ME: - g:=y; end process; fair process R \in{4,5} variable z=0; begin N: + z:=3; NE: - g:=z; end process; end algorithm; *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ 1 , 2 }) \cup {3} \cup ({ 4 , 5 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]
        /\ y = 0
        /\ z = [self \in { 4 , 5 } |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self = 3 -> "M" [] self \in { 4 , 5 } -> "N"]

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

Next == Q \/ (\E self \in { 1 , 2 }: P(self)) \/ (\E self \in { 4 , 5 }: R(self)) \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : WF_vars((pc[self] # "LE") /\ P(self)) /\ SF_vars(L(self))
        /\ WF_vars((pc[3] # "ME") /\ Q) /\ SF_vars(M)
        /\ \A self \in {4,5} : WF_vars((pc[self] # "NE") /\ R(self)) /\ SF_vars(N(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
