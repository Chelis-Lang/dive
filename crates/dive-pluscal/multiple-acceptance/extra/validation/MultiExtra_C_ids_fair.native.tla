---- MODULE MultiExtra_C_ids_fair ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; fair process (P =1) variable x=0; { L: x:=1; LE: g:=x; } fair process (Q =2) variable y=0; { M: y:=2; ME: g:=y; } fair process (R =3) variable z=0; { N: z:=3; NE: g:=z; } } *)

VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == {1} \cup {2} \cup {3}

Init == /\ g = 0
        /\ x = 0
        /\ y = 0
        /\ z = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M" [] self = 3 -> "N"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "LE"]
     /\ UNCHANGED << g, y, z >>

LE == /\ pc[1] = "LE"
      /\ g' = x
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << x, y, z >>

P == L \/ LE

M == /\ pc[2] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[2] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N == /\ pc[3] = "N"
     /\ z' = 3
     /\ pc' = [pc EXCEPT ![3] = "NE"]
     /\ UNCHANGED << g, x, y >>

NE == /\ pc[3] = "NE"
      /\ g' = z
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

R == N \/ NE

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ R \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(P)
        /\ WF_vars(Q)
        /\ WF_vars(R)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
