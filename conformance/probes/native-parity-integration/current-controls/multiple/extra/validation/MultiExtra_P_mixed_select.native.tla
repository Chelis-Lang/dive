---- MODULE MultiExtra_P_mixed_select ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=[a |-> 0]; begin L: x.a:=1; LE: g:=x.a; end process;  process Q =3 variable y=[a |-> 0]; begin M: y.a:=2; ME: g:=y.a; end process;  process R \in{4,5} variable z=[a |-> 0]; begin N: z.a:=3; NE: g:=z.a; end process; end algorithm; *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ 1 , 2 }) \cup {3} \cup ({ 4 , 5 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> [ a |-> 0 ]]
        /\ y = [ a |-> 0 ]
        /\ z = [self \in { 4 , 5 } |-> [ a |-> 0 ]]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self = 3 -> "M" [] self \in { 4 , 5 } -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self]. a = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self] . a
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ y' = [y EXCEPT !. a = 2]
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y . a
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ z' = [z EXCEPT ![self]. a = 3]
           /\ pc' = [pc EXCEPT ![self] = "NE"]
           /\ UNCHANGED << g, x, y >>

NE(self) == /\ pc[self] = "NE"
            /\ g' = z[self] . a
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

R(self) == N(self) \/ NE(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q \/ (\E self \in { 1 , 2 }: P(self)) \/ (\E self \in { 4 , 5 }: R(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
