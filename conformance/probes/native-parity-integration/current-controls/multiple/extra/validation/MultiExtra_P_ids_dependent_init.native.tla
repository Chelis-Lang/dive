---- MODULE MultiExtra_P_ids_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variables x=0, xd=x+1; begin L: x:=1; LE: g:=x; end process;  process Q =2 variables y=0, yd=y+1; begin M: y:=2; ME: g:=y; end process;  process R =3 variables z=0, zd=z+1; begin N: z:=3; NE: g:=z; end process; end algorithm; *)

VARIABLES pc, g, x, xd, y, yd, z, zd

vars == << pc, g, x, xd, y, yd, z, zd >>

ProcSet == {1} \cup {2} \cup {3}

Init == /\ g = 0
        /\ x = 0
        /\ xd = x + 1
        /\ y = 0
        /\ yd = y + 1
        /\ z = 0
        /\ zd = z + 1
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M" [] self = 3 -> "N"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "LE"]
     /\ UNCHANGED << g, xd, y, yd, z, zd >>

LE == /\ pc[1] = "LE"
      /\ g' = x
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << x, xd, y, yd, z, zd >>

P == L \/ LE

M == /\ pc[2] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "ME"]
     /\ UNCHANGED << g, x, xd, yd, z, zd >>

ME == /\ pc[2] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << x, xd, y, yd, z, zd >>

Q == M \/ ME

N == /\ pc[3] = "N"
     /\ z' = 3
     /\ pc' = [pc EXCEPT ![3] = "NE"]
     /\ UNCHANGED << g, x, xd, y, yd, zd >>

NE == /\ pc[3] = "NE"
      /\ g' = z
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, xd, y, yd, z, zd >>

R == N \/ NE

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ R \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
