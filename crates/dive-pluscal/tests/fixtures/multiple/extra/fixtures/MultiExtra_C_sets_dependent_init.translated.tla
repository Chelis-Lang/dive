---- MODULE MultiExtra_C_sets_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variables x=0, xd=x+1; { L: x:=1; LE: g:=x; }  process (Q \in{3,4}) variables y=0, yd=y+1; { M: y:=2; ME: g:=y; }  process (R \in{5}) variables z=0, zd=z+1; { N: z:=3; NE: g:=z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f5506826" /\ chksum(tla) = "751355a5")
VARIABLES pc, g, x, xd, y, yd, z, zd

vars == << pc, g, x, xd, y, yd, z, zd >>

ProcSet == ({1,2}) \cup ({3,4}) \cup ({5})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ xd = [self \in {1,2} |-> x[self]+1]
        (* Process Q *)
        /\ y = [self \in {3,4} |-> 0]
        /\ yd = [self \in {3,4} |-> y[self]+1]
        (* Process R *)
        /\ z = [self \in {5} |-> 0]
        /\ zd = [self \in {5} |-> z[self]+1]
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self \in {3,4} -> "M"
                                        [] self \in {5} -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, xd, y, yd, z, zd >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, xd, y, yd, z, zd >>

P(self) == L(self) \/ LE(self)

M(self) == /\ pc[self] = "M"
           /\ y' = [y EXCEPT ![self] = 2]
           /\ pc' = [pc EXCEPT ![self] = "ME"]
           /\ UNCHANGED << g, x, xd, yd, z, zd >>

ME(self) == /\ pc[self] = "ME"
            /\ g' = y[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, xd, y, yd, z, zd >>

Q(self) == M(self) \/ ME(self)

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

Next == (\E self \in {1,2}: P(self))
           \/ (\E self \in {3,4}: Q(self))
           \/ (\E self \in {5}: R(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
