---- MODULE MultiExtra_P_sets_labels ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin x:=1; LE: g:=x; end process;  process Q \in{3,4} variable y=0; begin y:=1; ME: g:=y; end process;  process R \in{5} variable z=0; begin z:=1; NE: g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1f249a05" /\ chksum(tla) = "e020c732")
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
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "Lbl_1"
                                        [] self \in {3,4} -> "Lbl_2"
                                        [] self \in {5} -> "Lbl_3"]

Lbl_1(self) == /\ pc[self] = "Lbl_1"
               /\ x' = [x EXCEPT ![self] = 1]
               /\ pc' = [pc EXCEPT ![self] = "LE"]
               /\ UNCHANGED << g, y, z >>

LE(self) == /\ pc[self] = "LE"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

P(self) == Lbl_1(self) \/ LE(self)

Lbl_2(self) == /\ pc[self] = "Lbl_2"
               /\ y' = [y EXCEPT ![self] = 1]
               /\ pc' = [pc EXCEPT ![self] = "ME"]
               /\ UNCHANGED << g, x, z >>

ME(self) == /\ pc[self] = "ME"
            /\ g' = y[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ UNCHANGED << x, y, z >>

Q(self) == Lbl_2(self) \/ ME(self)

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

Next == (\E self \in {1,2}: P(self))
           \/ (\E self \in {3,4}: Q(self))
           \/ (\E self \in {5}: R(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
