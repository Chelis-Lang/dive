---- MODULE MultiExtra_C_sets_member_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x \in {0,1}; { L: x:=1; LE: g:=x; }  process (Q \in{3,4}) variable y \in {0,1}; { M: y:=2; ME: g:=y; }  process (R \in{5}) variable z \in {0,1}; { N: z:=3; NE: g:=z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "dc599af9" /\ chksum(tla) = "a601b1ee")
VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({1,2}) \cup ({3,4}) \cup ({5})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x \in [{1,2} -> {0,1}]
        (* Process Q *)
        /\ y \in [{3,4} -> {0,1}]
        (* Process R *)
        /\ z \in [{5} -> {0,1}]
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self \in {3,4} -> "M"
                                        [] self \in {5} -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M(self) == /\ pc[self] = "M"
           /\ y' = [y EXCEPT ![self] = 2]
           /\ pc' = [pc EXCEPT ![self] = "ME"]
           /\ UNCHANGED << g, x, z >>

ME(self) == /\ pc[self] = "ME"
            /\ g' = y[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

Q(self) == M(self) \/ ME(self)

N(self) == /\ pc[self] = "N"
           /\ z' = [z EXCEPT ![self] = 3]
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

Next == (\E self \in {1,2}: P(self))
           \/ (\E self \in {3,4}: Q(self))
           \/ (\E self \in {5}: R(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
