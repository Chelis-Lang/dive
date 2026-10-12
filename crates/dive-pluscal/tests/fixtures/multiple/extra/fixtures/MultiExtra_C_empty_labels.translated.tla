---- MODULE MultiExtra_C_empty_labels ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { x:=1; LE: g:=x; }  process (Q =3) variable y=0; { y:=1; ME: g:=y; }  process (R \in{}) variable z=0; { z:=1; NE: g:=z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "384dcd52" /\ chksum(tla) = "c7ba552f")
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
        /\ pc = [self \in ProcSet |-> CASE self \in {} -> "Lbl_1"
                                        [] self = 3 -> "Lbl_2"
                                        [] self \in {} -> "Lbl_3"]

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
