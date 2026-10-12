---- MODULE MultiExtra_C_empty_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: x:=1 || g:=(x+1)%3; LE: skip; }  process (Q =3) variable y=0; { M: y:=2 || g:=(y+1)%3; ME: skip; }  process (R \in{}) variable z=0; { N: z:=3 || g:=(z+1)%3; NE: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "d9b9a63b" /\ chksum(tla) = "75d0e478")
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
           /\ /\ g' = (x[self]+1)%3
              /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ TRUE
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << g, x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ /\ g' = (y+1)%3
        /\ y' = 2
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << x, z >>

ME == /\ pc[3] = "ME"
      /\ TRUE
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << g, x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ /\ g' = (z[self]+1)%3
              /\ z' = [z EXCEPT ![self] = 3]
           /\ pc' = [pc EXCEPT ![self] = "NE"]
           /\ UNCHANGED << x, y >>

NE(self) == /\ pc[self] = "NE"
            /\ TRUE
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << g, x, y, z >>

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
