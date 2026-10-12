---- MODULE MultiExtra_P_empty_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{} variables x=0, xd=x+1; begin L: x:=1; LE: g:=x; end process;  process Q =3 variables y=0, yd=y+1; begin M: y:=2; ME: g:=y; end process;  process R \in{} variables z=0, zd=z+1; begin N: z:=3; NE: g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "81dad52a" /\ chksum(tla) = "90792b5b")
VARIABLES pc, g, x, xd, y, yd, z, zd

vars == << pc, g, x, xd, y, yd, z, zd >>

ProcSet == ({}) \cup {3} \cup ({})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {} |-> 0]
        /\ xd = [self \in {} |-> x[self]+1]
        (* Process Q *)
        /\ y = 0
        /\ yd = y+1
        (* Process R *)
        /\ z = [self \in {} |-> 0]
        /\ zd = [self \in {} |-> z[self]+1]
        /\ pc = [self \in ProcSet |-> CASE self \in {} -> "L"
                                        [] self = 3 -> "M"
                                        [] self \in {} -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, xd, y, yd, z, zd >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, xd, y, yd, z, zd >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, xd, yd, z, zd >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, xd, y, yd, z, zd >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ z' = [z EXCEPT ![self] = 3]
           /\ pc' = [pc EXCEPT ![self] = "NE"]
           /\ UNCHANGED << g, x, xd, y, yd, zd >>

NE(self) == /\ pc[self] = "NE"
            /\ g' = z[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, xd, y, yd, z, zd >>

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
