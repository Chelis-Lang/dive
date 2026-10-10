---- MODULE MultiExtra_C_ids_if ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: if (x=0) { x:=1; } else { x:=0; }; LE: g:=x; }  process (Q =2) variable y=0; { M: if (y=0) { y:=2; } else { y:=0; }; ME: g:=y; }  process (R =3) variable z=0; { N: if (z=0) { z:=3; } else { z:=0; }; NE: g:=z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "79d0bf5" /\ chksum(tla) = "7978c7cb")
VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == {1} \cup {2} \cup {3}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = 0
        (* Process R *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"
                                        [] self = 3 -> "N"]

L == /\ pc[1] = "L"
     /\ IF x=0
           THEN /\ x' = 1
           ELSE /\ x' = 0
     /\ pc' = [pc EXCEPT ![1] = "LE"]
     /\ UNCHANGED << g, y, z >>

LE == /\ pc[1] = "LE"
      /\ g' = x
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << x, y, z >>

P == L \/ LE

M == /\ pc[2] = "M"
     /\ IF y=0
           THEN /\ y' = 2
           ELSE /\ y' = 0
     /\ pc' = [pc EXCEPT ![2] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[2] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N == /\ pc[3] = "N"
     /\ IF z=0
           THEN /\ z' = 3
           ELSE /\ z' = 0
     /\ pc' = [pc EXCEPT ![3] = "NE"]
     /\ UNCHANGED << g, x, y >>

NE == /\ pc[3] = "NE"
      /\ g' = z
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

R == N \/ NE

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ R
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
