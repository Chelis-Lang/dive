---- MODULE MultiExtra_P_mixed_record_field ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: x:=[x |-> 1,y |-> 2,z |-> 3].x; LE: g:=x; end process;  process Q =3 variable y=0; begin M: y:=[x |-> 1,y |-> 2,z |-> 3].x; ME: g:=y; end process;  process R \in{4,5} variable z=0; begin N: z:=[x |-> 1,y |-> 2,z |-> 3].x; NE: g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8062bfc" /\ chksum(tla) = "116a974a")
VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == ({1,2}) \cup {3} \cup ({4,5})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        (* Process Q *)
        /\ y = 0
        (* Process R *)
        /\ z = [self \in {4,5} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self = 3 -> "M"
                                        [] self \in {4,5} -> "N"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = [x |-> 1,y |-> 2,z |-> 3].x]
           /\ pc' = [pc EXCEPT ![self] = "LE"]
           /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ LE(self)

M == /\ pc[3] = "M"
     /\ y' = [x |-> 1,y |-> 2,z |-> 3].x
     /\ pc' = [pc EXCEPT ![3] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[3] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N(self) == /\ pc[self] = "N"
           /\ z' = [z EXCEPT ![self] = [x |-> 1,y |-> 2,z |-> 3].x]
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
           \/ (\E self \in {1,2}: P(self))
           \/ (\E self \in {4,5}: R(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
