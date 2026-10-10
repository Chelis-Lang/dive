---- MODULE MultiExtra_P_mixed_labels ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin x:=1; LE: g:=x; end process;  process Q =3 variable y=0; begin y:=1; ME: g:=y; end process;  process R \in{4,5} variable z=0; begin z:=1; NE: g:=z; end process; end algorithm; *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({ 1 , 2 }) \cup {3} \cup ({ 4 , 5 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]
        /\ y = 0
        /\ z = [self \in { 4 , 5 } |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "Lbl_1" [] self = 3 -> "Lbl_2" [] self \in { 4 , 5 } -> "Lbl_3"]

Lbl_1(self) == /\ pc[self] = "Lbl_1"
               /\ x' = [x EXCEPT ![self] = 1]
               /\ pc' = [pc EXCEPT ![self] = "LE"]
               /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == Lbl_1(self) \/ LE(self)

Lbl_2 == /\ pc[3] = "Lbl_2"
         /\ y' = 1
         /\ pc' = [pc EXCEPT ![3] = "ME"]
         /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == Lbl_2 \/ ME

Lbl_3(self) == /\ pc[self] = "Lbl_3"
               /\ z' = [z EXCEPT ![self] = 1]
               /\ pc' = [pc EXCEPT ![self] = "NE"]
               /\ UNCHANGED << g, x, y >>

NE(self) == /\ pc[self] = "NE"
            /\ g' = z[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

R(self) == Lbl_3(self) \/ NE(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q \/ (\E self \in { 1 , 2 }: P(self)) \/ (\E self \in { 4 , 5 }: R(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
