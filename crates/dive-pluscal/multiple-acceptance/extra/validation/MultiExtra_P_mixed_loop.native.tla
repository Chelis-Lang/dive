---- MODULE MultiExtra_P_mixed_loop ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: while x<2 do x:=x+1; end while; LE: g:=x; end process;  process Q =3 variable y=0; begin M: while y<2 do y:=y+1; end while; ME: g:=y; end process;  process R \in{4,5} variable z=0; begin N: while z<2 do z:=z+1; end while; NE: g:=z; end process; end algorithm; *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ 1 , 2 }) \cup {3} \cup ({ 4 , 5 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]
        /\ y = 0
        /\ z = [self \in { 4 , 5 } |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self = 3 -> "M" [] self \in { 4 , 5 } -> "N"]

L(self) == /\ pc[self] = "L"
           /\ IF x[self] < 2
                 THEN /\ x' = [x EXCEPT ![self] = x[self] + 1]
                      /\ pc' = [pc EXCEPT ![self] = "L"]
                 ELSE /\ pc' = [pc EXCEPT ![self] = "LE"]
                      /\ x' = x
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ IF y < 2
           THEN /\ y' = y + 1
                /\ pc' = [pc EXCEPT ![3] = "M"]
           ELSE /\ pc' = [pc EXCEPT ![3] = "ME"]
                /\ y' = y
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ IF z[self] < 2
                 THEN /\ z' = [z EXCEPT ![self] = z[self] + 1]
                      /\ pc' = [pc EXCEPT ![self] = "N"]
                 ELSE /\ pc' = [pc EXCEPT ![self] = "NE"]
                      /\ z' = z
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
