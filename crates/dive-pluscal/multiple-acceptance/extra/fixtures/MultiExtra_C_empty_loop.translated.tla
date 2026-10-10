---- MODULE MultiExtra_C_empty_loop ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: while (x<2) { x:=x+1; }; LE: g:=x; }  process (Q =3) variable y=0; { M: while (y<2) { y:=y+1; }; ME: g:=y; }  process (R \in{}) variable z=0; { N: while (z<2) { z:=z+1; }; NE: g:=z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "8566b49c" /\ chksum(tla) = "5624adca")
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
           /\ IF x[self]<2
                 THEN /\ x' = [x EXCEPT ![self] = x[self]+1]
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
     /\ IF y<2
           THEN /\ y' = y+1
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
           /\ IF z[self]<2
                 THEN /\ z' = [z EXCEPT ![self] = z[self]+1]
                      /\ pc' = [pc EXCEPT ![self] = "N"]
                 ELSE /\ pc' = [pc EXCEPT ![self] = "NE"]
                      /\ z' = z
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
