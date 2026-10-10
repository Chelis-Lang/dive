---- MODULE MultiExtra_C_empty_if ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: if (x=0) { x:=1; } else { x:=0; }; LE: g:=x; }  process (Q =3) variable y=0; { M: if (y=0) { y:=2; } else { y:=0; }; ME: g:=y; }  process (R \in{}) variable z=0; { N: if (z=0) { z:=3; } else { z:=0; }; NE: g:=z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "3b4b204" /\ chksum(tla) = "4efad322")
VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({}) \cup {3} \cup ({})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {} |-> 0]
        (* Process Q *)
        /\ y = 0
        (* Process R *)
        /\ z = [self \in {} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in {} -> "L"
                                        [] self = 3 -> "M"
                                        [] self \in {} -> "N"]

L(self) == /\ pc[self] = "L"
           /\ IF x[self]=0
                 THEN /\ x' = [x EXCEPT ![self] = 1]
                 ELSE /\ x' = [x EXCEPT ![self] = 0]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ IF y=0
           THEN /\ y' = 2
           ELSE /\ y' = 0
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ IF z[self]=0
                 THEN /\ z' = [z EXCEPT ![self] = 3]
                 ELSE /\ z' = [z EXCEPT ![self] = 0]
           /\ pc' = [pc EXCEPT ![self] = "NE"]
           /\ UNCHANGED << g, x, y >>

NE(self) == /\ pc[self] = "NE"
            /\ g' = z[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

R(self) == N(self) \/ NE(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q
           \/ (\E self \in {}: P(self))
           \/ (\E self \in {}: R(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
