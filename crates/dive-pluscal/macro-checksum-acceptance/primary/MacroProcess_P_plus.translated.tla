---- MODULE MacroProcess_P_plus ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=a; end macro; process P\in {1,2} variable z=0; begin L: + Put(z,self); E: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "415bdd1f" /\ chksum(tla) = "1f38108d")
VARIABLES pc, x, y, z

vars == << pc, x, y, z >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        (* Process P *)
        /\ z = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ z' = [z EXCEPT ![self] = self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << x, y >>

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
