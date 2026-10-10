---- MODULE MultiExtra_P_ids_select ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=[a |-> 0]; begin L: x.a:=1; LE: g:=x.a; end process;  process Q =2 variable y=[a |-> 0]; begin M: y.a:=2; ME: g:=y.a; end process;  process R =3 variable z=[a |-> 0]; begin N: z.a:=3; NE: g:=z.a; end process; end algorithm; *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == {1} \cup {2} \cup {3}

Init == /\ g = 0
        /\ x = [ a |-> 0 ]
        /\ y = [ a |-> 0 ]
        /\ z = [ a |-> 0 ]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M" [] self = 3 -> "N"]

L == /\ pc[1] = "L"
     /\ x' = [x EXCEPT !. a = 1]
     /\ pc' = [pc EXCEPT ![1] = "LE"]
     /\ UNCHANGED << g, y, z >>

LE == /\ pc[1] = "LE"
      /\ g' = x . a
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << x, y, z >>

P == L \/ LE

M == /\ pc[2] = "M"
     /\ y' = [y EXCEPT !. a = 2]
     /\ pc' = [pc EXCEPT ![2] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[2] = "ME"
      /\ g' = y . a
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N == /\ pc[3] = "N"
     /\ z' = [z EXCEPT !. a = 3]
     /\ pc' = [pc EXCEPT ![3] = "NE"]
     /\ UNCHANGED << g, x, y >>

NE == /\ pc[3] = "NE"
      /\ g' = z . a
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

R == N \/ NE

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ R \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
