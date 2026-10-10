---- MODULE MultiExtra_P_sets_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: x:=1 || g:=(x+1)%3; LE: skip; end process;  process Q \in{3,4} variable y=0; begin M: y:=2 || g:=(y+1)%3; ME: skip; end process;  process R \in{5} variable z=0; begin N: z:=3 || g:=(z+1)%3; NE: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a1bae517" /\ chksum(tla) = "fef97e9f")
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
           /\ /\ g' = (x[self]+1)%3
              /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ TRUE
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << g, x, y, z >>

P(self) == L(self) \/ LE(self)

M(self) == /\ pc[self] = "M"
           /\ /\ g' = (y[self]+1)%3
              /\ y' = [y EXCEPT ![self] = 2]
           /\ pc' = [pc EXCEPT ![self] = "ME"]
           /\ UNCHANGED << x, z >>

ME(self) == /\ pc[self] = "ME"
            /\ TRUE
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << g, x, y, z >>

Q(self) == M(self) \/ ME(self)

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

Next == (\E self \in {1,2}: P(self))
           \/ (\E self \in {3,4}: Q(self))
           \/ (\E self \in {5}: R(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
