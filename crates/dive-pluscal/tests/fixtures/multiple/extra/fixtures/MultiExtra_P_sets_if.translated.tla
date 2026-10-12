---- MODULE MultiExtra_P_sets_if ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: if x=0 then x:=1; else x:=0; end if; LE: g:=x; end process;  process Q \in{3,4} variable y=0; begin M: if y=0 then y:=2; else y:=0; end if; ME: g:=y; end process;  process R \in{5} variable z=0; begin N: if z=0 then z:=3; else z:=0; end if; NE: g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5975509d" /\ chksum(tla) = "293eddfa")
VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({1,2}) \cup ({3,4}) \cup ({5})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        (* Process Q *)
        /\ y = [self \in {3,4} |-> 0]
        (* Process R *)
        /\ z = [self \in {5} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self \in {3,4} -> "M"
                                        [] self \in {5} -> "N"]

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

M(self) == /\ pc[self] = "M"
           /\ IF y[self]=0
                 THEN /\ y' = [y EXCEPT ![self] = 2]
                 ELSE /\ y' = [y EXCEPT ![self] = 0]
           /\ pc' = [pc EXCEPT ![self] = "ME"]
           /\ UNCHANGED << g, x, z >>

ME(self) == /\ pc[self] = "ME"
            /\ g' = y[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

Q(self) == M(self) \/ ME(self)

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

Next == (\E self \in {1,2}: P(self))
           \/ (\E self \in {3,4}: Q(self))
           \/ (\E self \in {5}: R(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
