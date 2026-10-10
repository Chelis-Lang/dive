---- MODULE MultiExtra_P_sets_select ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=[a |-> 0]; begin L: x.a:=1; LE: g:=x.a; end process;  process Q \in{3,4} variable y=[a |-> 0]; begin M: y.a:=2; ME: g:=y.a; end process;  process R \in{5} variable z=[a |-> 0]; begin N: z.a:=3; NE: g:=z.a; end process; end algorithm; *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ 1 , 2 }) \cup ({ 3 , 4 }) \cup ({ 5 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> [ a |-> 0 ]]
        /\ y = [self \in { 3 , 4 } |-> [ a |-> 0 ]]
        /\ z = [self \in { 5 } |-> [ a |-> 0 ]]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self \in { 3 , 4 } -> "M" [] self \in { 5 } -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self]. a = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self] . a
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M(self) == /\ pc[self] = "M"
           /\ y' = [y EXCEPT ![self]. a = 2]
           /\ pc' = [pc EXCEPT ![self] = "ME"]
           /\ UNCHANGED << g, x, z >>

ME(self) == /\ pc[self] = "ME"
            /\ g' = y[self] . a
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

Q(self) == M(self) \/ ME(self)

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

Next == (\E self \in { 1 , 2 }: P(self)) \/ (\E self \in { 3 , 4 }: Q(self)) \/ (\E self \in { 5 }: R(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
